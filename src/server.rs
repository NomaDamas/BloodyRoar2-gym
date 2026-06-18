use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use crate::action::Action;
use crate::backend::{Backend, BackendError, NullBackend};
use crate::env::BloodyRoar2Env;
use crate::native::NativeBackend;
use crate::protocol::{action_space_json, api_index_json, observation_space_json};

pub fn serve(address: &str) -> Result<(), BackendError> {
    serve_with_backend(address, NullBackend::default())
}

pub fn serve_native(
    address: &str,
    rom_path: impl Into<std::path::PathBuf>,
    instructions_per_frame: u64,
) -> Result<(), BackendError> {
    let backend = NativeBackend::from_rom_zip(rom_path, instructions_per_frame)?;
    serve_with_backend(address, backend)
}

fn serve_with_backend<B>(address: &str, backend: B) -> Result<(), BackendError>
where
    B: Backend + Send + 'static,
{
    let listener = TcpListener::bind(address)
        .map_err(|error| BackendError::new(format!("failed to bind {address}: {error}")))?;
    let env = Arc::new(Mutex::new(BloodyRoar2Env::new(backend)));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let env = Arc::clone(&env);
                std::thread::spawn(move || {
                    let _ = handle_client(stream, env);
                });
            }
            Err(error) => eprintln!("connection error: {error}"),
        }
    }

    Ok(())
}

fn handle_client<B>(
    mut stream: TcpStream,
    env: Arc<Mutex<BloodyRoar2Env<B>>>,
) -> Result<(), BackendError>
where
    B: Backend,
{
    let mut buffer = [0_u8; 4096];
    let read = stream
        .read(&mut buffer)
        .map_err(|error| BackendError::new(format!("failed to read request: {error}")))?;
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first_line = request.lines().next().unwrap_or_default();

    let response = if first_line.starts_with("GET / ") {
        ok(api_index_json())
    } else if first_line.starts_with("GET /action_space ") {
        ok(action_space_json())
    } else if first_line.starts_with("GET /observation_space ") {
        ok(observation_space_json())
    } else if first_line.starts_with("POST /reset ") {
        let mut env = env
            .lock()
            .map_err(|_| BackendError::new("environment lock poisoned"))?;
        match env.reset() {
            Ok(observation) => ok(format!(
                "{{\"observation\":{},\"info\":{{}}}}",
                observation.json()
            )),
            Err(error) => internal_error(error.to_string()),
        }
    } else if first_line.starts_with("POST /step ") {
        let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        let action_index = parse_number_field(body, "action").unwrap_or(0) as usize;
        let frames = parse_number_field(body, "frames").unwrap_or(1) as u32;
        match Action::from_index(action_index) {
            Some(action) => {
                let mut env = env
                    .lock()
                    .map_err(|_| BackendError::new("environment lock poisoned"))?;
                match env.step(action, frames) {
                    Ok(step) => ok(step.json()),
                    Err(error) => internal_error(error.to_string()),
                }
            }
            None => bad_request(format!(
                "{{\"error\":\"action must be between 0 and {}\"}}",
                crate::ACTION_SPACE.len() - 1
            )),
        }
    } else {
        not_found("{\"error\":\"not found\"}".to_string())
    };

    stream
        .write_all(response.as_bytes())
        .map_err(|error| BackendError::new(format!("failed to write response: {error}")))?;
    Ok(())
}

fn parse_number_field(body: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\"");
    let start = body.find(&needle)?;
    let after_key = &body[start + needle.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let digits = after_colon
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn ok(body: String) -> String {
    response("200 OK", body)
}

fn bad_request(body: String) -> String {
    response("400 Bad Request", body)
}

fn not_found(body: String) -> String {
    response("404 Not Found", body)
}

fn internal_error(message: String) -> String {
    response(
        "500 Internal Server Error",
        format!("{{\"error\":\"{}\"}}", message.replace('"', "'")),
    )
}

fn response(status: &str, body: String) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}
