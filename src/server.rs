use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::action::Action;
use crate::backend::{Backend, BackendError, NullBackend};
use crate::env::BloodyRoar2Env;
use crate::native::NativeBackend;
use crate::protocol::{action_space_json, api_index_json, observation_space_json};

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const CLIENT_IO_TIMEOUT: Duration = Duration::from_secs(5);

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
    stream
        .set_read_timeout(Some(CLIENT_IO_TIMEOUT))
        .map_err(|error| BackendError::new(format!("failed to set read timeout: {error}")))?;
    stream
        .set_write_timeout(Some(CLIENT_IO_TIMEOUT))
        .map_err(|error| BackendError::new(format!("failed to set write timeout: {error}")))?;
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(message) => {
            stream
                .write_all(bad_request(error_json(&message)).as_bytes())
                .map_err(|error| BackendError::new(format!("failed to write response: {error}")))?;
            return Ok(());
        }
    };
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
        match parse_step_request(body) {
            Ok((action_index, frames)) => match Action::from_index(action_index) {
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
            },
            Err(message) => bad_request(error_json(&message)),
        }
    } else {
        not_found("{\"error\":\"not found\"}".to_string())
    };

    stream
        .write_all(response.as_bytes())
        .map_err(|error| BackendError::new(format!("failed to write response: {error}")))?;
    Ok(())
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected_len = None;

    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err(format!(
                "request exceeds maximum size of {MAX_REQUEST_BYTES} bytes"
            ));
        }

        if expected_len.is_none()
            && let Some(header_end) = find_header_end(&request)
        {
            let headers = std::str::from_utf8(&request[..header_end])
                .map_err(|_| "request headers must be valid UTF-8".to_string())?;
            let content_length = parse_content_length(headers)?;
            expected_len = Some(
                header_end
                    .checked_add(4)
                    .and_then(|length| length.checked_add(content_length))
                    .ok_or_else(|| "request length overflow".to_string())?,
            );
        }

        if expected_len.is_some_and(|expected_len| request.len() >= expected_len) {
            break;
        }
    }

    let expected_len = expected_len.ok_or_else(|| "incomplete HTTP headers".to_string())?;
    if request.len() < expected_len {
        return Err("incomplete HTTP request body".to_string());
    }
    request.truncate(expected_len);
    String::from_utf8(request).map_err(|_| "request must be valid UTF-8".to_string())
}

fn find_header_end(request: &[u8]) -> Option<usize> {
    request.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(headers: &str) -> Result<usize, String> {
    for line in headers.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            return Err("malformed HTTP header".to_string());
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            return value
                .trim()
                .parse::<usize>()
                .map_err(|_| "invalid Content-Length".to_string());
        }
    }
    Ok(0)
}

fn parse_step_request(body: &str) -> Result<(usize, u32), String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON body: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "request body must be a JSON object".to_string())?;
    let action = object
        .get("action")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "action must be a non-negative integer".to_string())?;
    let action =
        usize::try_from(action).map_err(|_| "action is outside the supported range".to_string())?;
    let frames = object
        .get("frames")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| "frames must be a positive integer".to_string())
        })
        .transpose()?
        .unwrap_or(1);
    if frames == 0 || frames > u32::MAX as u64 {
        return Err(format!("frames must be between 1 and {}", u32::MAX));
    }
    Ok((action, frames as u32))
}

fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
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
    response("500 Internal Server Error", error_json(&message))
}

fn response(status: &str, body: String) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

#[cfg(test)]
mod tests {
    use super::{find_header_end, internal_error, parse_content_length, parse_step_request};

    #[test]
    fn step_request_requires_valid_integer_action_and_positive_frames() {
        assert_eq!(parse_step_request(r#"{"action":5}"#), Ok((5, 1)));
        assert_eq!(parse_step_request(r#"{"action":5,"frames":4}"#), Ok((5, 4)));
        assert!(parse_step_request(r#"{}"#).is_err());
        assert!(parse_step_request(r#"{"action":"5"}"#).is_err());
        assert!(parse_step_request(r#"{"action":-1}"#).is_err());
        assert!(parse_step_request(r#"{"action":5,"frames":0}"#).is_err());
        assert!(parse_step_request(r#"{"action":5"#).is_err());
    }

    #[test]
    fn http_header_parsing_is_case_insensitive_and_bounded_by_body_length() {
        let request = b"POST /step HTTP/1.1\r\ncontent-length: 12\r\n\r\n{}";
        let header_end = find_header_end(request).expect("header end");
        assert_eq!(
            parse_content_length(std::str::from_utf8(&request[..header_end]).unwrap()),
            Ok(12)
        );
    }

    #[test]
    fn internal_errors_escape_json_control_characters() {
        let response = internal_error("bad \"path\"\\name\nnext".to_string());
        let body = response.split("\r\n\r\n").nth(1).expect("response body");
        let parsed: serde_json::Value = serde_json::from_str(body).expect("valid JSON");
        assert_eq!(parsed["error"], "bad \"path\"\\name\nnext");
    }
}
