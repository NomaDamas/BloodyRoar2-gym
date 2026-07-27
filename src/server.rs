use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::action::{Action, ActionButtons};
use crate::backend::{Backend, BackendError, NullBackend};
use crate::env::{BloodyRoar2Env, MAX_STEP_FRAMES};
use crate::native::NativeBackend;
use crate::protocol::{action_space_json, api_index_json, observation_space_json};

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const CLIENT_IO_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StepControl {
    Action(usize),
    Buttons(ActionButtons),
}

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
    let response = route_request(&request, &env)?;

    stream
        .write_all(response.as_bytes())
        .map_err(|error| BackendError::new(format!("failed to write response: {error}")))?;
    Ok(())
}

fn route_request<B>(
    request: &str,
    env: &Arc<Mutex<BloodyRoar2Env<B>>>,
) -> Result<String, BackendError>
where
    B: Backend,
{
    let first_line = request.lines().next().unwrap_or_default();
    let response = if first_line.starts_with("GET / ") {
        ok(api_index_json())
    } else if first_line.starts_with("GET /action_space ") {
        ok(action_space_json())
    } else if first_line.starts_with("GET /observation_space ") {
        ok(observation_space_json())
    } else if first_line.starts_with("POST /reset ") {
        let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        match parse_observation_options(body) {
            Ok(include_screenshot) => {
                let mut env = env
                    .lock()
                    .map_err(|_| BackendError::new("environment lock poisoned"))?;
                env.set_observation_screenshot(include_screenshot);
                match env.reset() {
                    Ok(observation) => ok(format!(
                        "{{\"observation\":{},\"info\":{}}}",
                        observation.json(),
                        observation.info_json
                    )),
                    Err(error) => internal_error(error.to_string()),
                }
            }
            Err(message) => bad_request(error_json(&message)),
        }
    } else if first_line.starts_with("POST /step ") {
        let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
        match parse_step_request(body) {
            Ok((control, frames, include_screenshot)) => {
                let buttons = match control {
                    StepControl::Action(action_index) => match Action::from_index(action_index) {
                        Some(action) => action.buttons(),
                        None => {
                            return Ok(bad_request(format!(
                                "{{\"error\":\"action must be between 0 and {}\"}}",
                                crate::ACTION_SPACE.len() - 1
                            )));
                        }
                    },
                    StepControl::Buttons(buttons) => buttons,
                };
                {
                    let mut env = env
                        .lock()
                        .map_err(|_| BackendError::new("environment lock poisoned"))?;
                    env.set_observation_screenshot(include_screenshot);
                    match env.step_buttons(buttons, frames) {
                        Ok(step) => ok(step.json()),
                        Err(error) => internal_error(error.to_string()),
                    }
                }
            }
            Err(message) => bad_request(error_json(&message)),
        }
    } else {
        not_found("{\"error\":\"not found\"}".to_string())
    };
    Ok(response)
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

fn parse_step_request(body: &str) -> Result<(StepControl, u32, bool), String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON body: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "request body must be a JSON object".to_string())?;
    let action = object.get("action");
    let buttons = object.get("buttons");
    let control = match (action, buttons) {
        (Some(_), Some(_)) => {
            return Err("provide exactly one of action or buttons, not both".to_string());
        }
        (None, None) => return Err("provide exactly one of action or buttons".to_string()),
        (Some(action), None) => {
            let action = action
                .as_u64()
                .ok_or_else(|| "action must be a non-negative integer".to_string())?;
            let action = usize::try_from(action)
                .map_err(|_| "action is outside the supported range".to_string())?;
            StepControl::Action(action)
        }
        (None, Some(buttons)) => StepControl::Buttons(parse_action_buttons(buttons)?),
    };
    let frames = object
        .get("frames")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| "frames must be a positive integer".to_string())
        })
        .transpose()?
        .unwrap_or(1);
    if frames == 0 || frames > MAX_STEP_FRAMES as u64 {
        return Err(format!("frames must be between 1 and {MAX_STEP_FRAMES}"));
    }
    let include_screenshot = parse_screenshot_option(object)?;
    Ok((control, frames as u32, include_screenshot))
}

fn parse_action_buttons(value: &serde_json::Value) -> Result<ActionButtons, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "buttons must be a JSON object".to_string())?;
    let allowed = [
        "start", "coin", "service", "up", "down", "left", "right", "punch", "kick", "beast",
        "guard",
    ];
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unknown buttons field: {key}"));
    }
    let read = |name: &str| -> Result<bool, String> {
        object
            .get(name)
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| format!("buttons.{name} must be a boolean"))
            })
            .transpose()
            .map(|value| value.unwrap_or(false))
    };
    Ok(ActionButtons {
        start: read("start")?,
        coin: read("coin")?,
        service: read("service")?,
        up: read("up")?,
        down: read("down")?,
        left: read("left")?,
        right: read("right")?,
        punch: read("punch")?,
        kick: read("kick")?,
        beast: read("beast")?,
        guard: read("guard")?,
    })
}

fn parse_observation_options(body: &str) -> Result<bool, String> {
    if body.trim().is_empty() {
        return Ok(false);
    }
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|error| format!("invalid JSON body: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "request body must be a JSON object".to_string())?;
    parse_screenshot_option(object)
}

fn parse_screenshot_option(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<bool, String> {
    object
        .get("screenshot")
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| "screenshot must be a boolean".to_string())
        })
        .transpose()
        .map(|value| value.unwrap_or(false))
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
    use std::sync::{Arc, Mutex};

    use crate::action::ActionButtons;
    use crate::backend::{Backend, BackendError, Observation};
    use crate::env::BloodyRoar2Env;

    use super::{
        StepControl, find_header_end, internal_error, parse_content_length,
        parse_observation_options, parse_step_request, route_request,
    };

    #[derive(Default)]
    struct ScreenshotBackend {
        frame: u64,
        include_screenshot: bool,
    }

    impl ScreenshotBackend {
        fn observation(&self) -> Observation {
            Observation {
                frame: self.frame,
                player_health: 1.0,
                opponent_health: 1.0,
                beast_meter: 0.0,
                round_time: 99.0,
                terminal: false,
                screenshot_b64: self.include_screenshot.then(|| "test-png".to_string()),
                info_json: format!("{{\"frame\":{}}}", self.frame),
            }
        }
    }

    impl Backend for ScreenshotBackend {
        fn set_observation_screenshot(&mut self, enabled: bool) {
            self.include_screenshot = enabled;
        }

        fn reset(&mut self) -> Result<Observation, BackendError> {
            self.frame = 0;
            Ok(self.observation())
        }

        fn step(
            &mut self,
            _buttons: ActionButtons,
            frames: u32,
        ) -> Result<Observation, BackendError> {
            self.frame = self.frame.saturating_add(frames.max(1) as u64);
            Ok(self.observation())
        }
    }

    fn response_json(response: &str) -> serde_json::Value {
        let body = response
            .split("\r\n\r\n")
            .nth(1)
            .expect("HTTP response body");
        serde_json::from_str(body).expect("valid JSON response")
    }

    #[test]
    fn step_request_requires_valid_integer_action_and_positive_frames() {
        assert_eq!(
            parse_step_request(r#"{"action":5}"#),
            Ok((StepControl::Action(5), 1, false))
        );
        assert_eq!(
            parse_step_request(r#"{"action":5,"frames":4}"#),
            Ok((StepControl::Action(5), 4, false))
        );
        assert_eq!(
            parse_step_request(r#"{"action":5,"frames":4,"screenshot":true}"#),
            Ok((StepControl::Action(5), 4, true))
        );
        assert_eq!(
            parse_step_request(r#"{"action":5,"screenshot":false}"#),
            Ok((StepControl::Action(5), 1, false))
        );
        assert_eq!(
            parse_step_request(r#"{"buttons":{"up":true,"punch":true,"guard":true},"frames":2}"#),
            Ok((
                StepControl::Buttons(ActionButtons {
                    up: true,
                    punch: true,
                    guard: true,
                    ..ActionButtons::default()
                }),
                2,
                false
            ))
        );
        assert!(parse_step_request(r#"{}"#).is_err());
        assert!(parse_step_request(r#"{"action":5,"buttons":{}}"#).is_err());
        assert!(parse_step_request(r#"{"action":"5"}"#).is_err());
        assert!(parse_step_request(r#"{"action":-1}"#).is_err());
        assert!(parse_step_request(r#"{"action":5,"frames":0}"#).is_err());
        assert!(
            parse_step_request(&format!(
                r#"{{"action":5,"frames":{}}}"#,
                crate::env::MAX_STEP_FRAMES + 1
            ))
            .is_err()
        );
        assert!(parse_step_request(r#"{"buttons":{"up":1}}"#).is_err());
        assert!(parse_step_request(r#"{"buttons":{"turbo":true}}"#).is_err());
        assert!(parse_step_request(r#"{"action":5,"screenshot":1}"#).is_err());
        assert!(parse_step_request(r#"{"action":5"#).is_err());
    }

    #[test]
    fn reset_observation_options_default_to_lightweight_responses() {
        assert_eq!(parse_observation_options(""), Ok(false));
        assert_eq!(parse_observation_options("{}"), Ok(false));
        assert_eq!(
            parse_observation_options(r#"{"screenshot":true}"#),
            Ok(true)
        );
        assert!(parse_observation_options(r#"{"screenshot":"yes"}"#).is_err());
    }

    #[test]
    fn http_requests_enable_and_clear_screenshot_observations() {
        let env = Arc::new(Mutex::new(
            BloodyRoar2Env::new(ScreenshotBackend::default()),
        ));

        let reset_with_image = route_request(
            "POST /reset HTTP/1.1\r\nContent-Length: 19\r\n\r\n{\"screenshot\":true}",
            &env,
        )
        .expect("reset with screenshot");
        assert_eq!(
            response_json(&reset_with_image)["observation"]["screenshot_b64"],
            "test-png"
        );
        assert_eq!(response_json(&reset_with_image)["info"]["frame"], 0);

        let step_without_image = route_request(
            "POST /step HTTP/1.1\r\nContent-Length: 12\r\n\r\n{\"action\":5}",
            &env,
        )
        .expect("step without screenshot");
        assert!(
            response_json(&step_without_image)["observation"]["screenshot_b64"].is_null(),
            "a request without screenshot must clear the prior true setting"
        );

        let step_with_image = route_request(
            "POST /step HTTP/1.1\r\nContent-Length: 30\r\n\r\n{\"action\":5,\"screenshot\":true}",
            &env,
        )
        .expect("step with screenshot");
        assert_eq!(
            response_json(&step_with_image)["observation"]["screenshot_b64"],
            "test-png"
        );
        assert_eq!(response_json(&step_with_image)["info"]["frame"], 2);

        let step_with_buttons = route_request(
            "POST /step HTTP/1.1\r\nContent-Length: 50\r\n\r\n{\"buttons\":{\"up\":true,\"punch\":true},\"frames\":3}",
            &env,
        )
        .expect("step with simultaneous buttons");
        assert_eq!(response_json(&step_with_buttons)["info"]["frame"], 5);

        let reset_without_image =
            route_request("POST /reset HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}", &env)
                .expect("reset without screenshot");
        assert!(
            response_json(&reset_without_image)["observation"]["screenshot_b64"].is_null(),
            "reset without screenshot must clear the prior true setting"
        );
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
