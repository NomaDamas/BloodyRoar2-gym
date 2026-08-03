use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use serde_json::{Map, Value, json};

use crate::action::{Action, ActionButtons};
use crate::backend::{BackendError, NullBackend, Observation};
use crate::env::{BloodyRoar2Env, MAX_STEP_FRAMES, StepResult};
use crate::moves::{
    ActionSegment, BLOODY_ROAR_2_ROSTER, Facing, MAX_ACTION_SEQUENCE_SEGMENTS, NAMED_ACTIONS,
    Player, named_action_sequence,
};
use crate::native::NativeBackend;
use crate::protocol::{action_space_json, character_action_space_json, observation_space_json};
use crate::server::{parse_action_buttons, parse_action_segments};

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MCP_SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] =
    [MCP_PROTOCOL_VERSION, "2025-06-18", "2024-11-05"];

enum McpBackendSource {
    Null,
    Native {
        rom_path: PathBuf,
        instructions_per_frame: u64,
    },
}

enum McpEnvironment {
    Null(BloodyRoar2Env<NullBackend>),
    Native(Box<BloodyRoar2Env<NativeBackend>>),
}

impl McpEnvironment {
    fn set_observation_screenshot(&mut self, enabled: bool) {
        match self {
            Self::Null(env) => env.set_observation_screenshot(enabled),
            Self::Native(env) => env.set_observation_screenshot(enabled),
        }
    }

    fn reset(&mut self) -> Result<Observation, BackendError> {
        match self {
            Self::Null(env) => env.reset(),
            Self::Native(env) => env.reset(),
        }
    }

    fn reset_match(
        &mut self,
        p1_character: &str,
        p2_character: &str,
    ) -> Result<Observation, BackendError> {
        match self {
            Self::Null(env) => env.reset_match(p1_character, p2_character),
            Self::Native(env) => env.reset_match(p1_character, p2_character),
        }
    }

    fn active_characters(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Null(env) => env.active_characters(),
            Self::Native(env) => env.active_characters(),
        }
    }

    fn ensure_character_selected(
        &mut self,
        character: &str,
        player: Player,
    ) -> Result<bool, BackendError> {
        match self {
            Self::Null(env) => env.ensure_character_selected(character, player),
            Self::Native(env) => env.ensure_character_selected(character, player),
        }
    }

    fn observe(&mut self) -> Result<Observation, BackendError> {
        match self {
            Self::Null(env) => env.observe(),
            Self::Native(env) => env.observe(),
        }
    }

    fn step(&mut self, action: Action, frames: u32) -> Result<StepResult, BackendError> {
        match self {
            Self::Null(env) => env.step(action, frames),
            Self::Native(env) => env.step(action, frames),
        }
    }

    fn step_buttons(
        &mut self,
        buttons: ActionButtons,
        frames: u32,
    ) -> Result<StepResult, BackendError> {
        match self {
            Self::Null(env) => env.step_buttons(buttons, frames),
            Self::Native(env) => env.step_buttons(buttons, frames),
        }
    }

    fn step_sequence(
        &mut self,
        segments: &[ActionSegment],
        include_screenshot: bool,
    ) -> Result<StepResult, BackendError> {
        match self {
            Self::Null(env) => env.step_sequence(segments, include_screenshot),
            Self::Native(env) => env.step_sequence(segments, include_screenshot),
        }
    }

    fn last_observation(&self) -> Option<&Observation> {
        match self {
            Self::Null(env) => env.last_observation(),
            Self::Native(env) => env.last_observation(),
        }
    }
}

struct McpRuntime {
    source: McpBackendSource,
    environment: Option<McpEnvironment>,
}

impl McpRuntime {
    fn null() -> Self {
        Self {
            source: McpBackendSource::Null,
            environment: None,
        }
    }

    fn native(rom_path: PathBuf, instructions_per_frame: u64) -> Self {
        Self {
            source: McpBackendSource::Native {
                rom_path,
                instructions_per_frame: instructions_per_frame.max(1),
            },
            environment: None,
        }
    }

    fn backend_name(&self) -> &'static str {
        match self.source {
            McpBackendSource::Null => "null",
            McpBackendSource::Native { .. } => "native",
        }
    }

    fn ensure_environment(&mut self) -> Result<&mut McpEnvironment, BackendError> {
        if self.environment.is_none() {
            let environment = match &self.source {
                McpBackendSource::Null => {
                    McpEnvironment::Null(BloodyRoar2Env::new(NullBackend::default()))
                }
                McpBackendSource::Native {
                    rom_path,
                    instructions_per_frame,
                } => McpEnvironment::Native(Box::new(BloodyRoar2Env::new(
                    NativeBackend::from_rom_zip(rom_path.clone(), *instructions_per_frame)?,
                ))),
            };
            self.environment = Some(environment);
        }
        self.environment
            .as_mut()
            .ok_or_else(|| BackendError::new("MCP environment failed to initialize"))
    }

    fn health(&self) -> Value {
        let observation = self
            .environment
            .as_ref()
            .and_then(McpEnvironment::last_observation)
            .map(observation_value);
        json!({
            "status": "ok",
            "backend": self.backend_name(),
            "loaded": self.environment.is_some(),
            "protocol_version": MCP_PROTOCOL_VERSION,
            "action_count": crate::ACTION_SPACE.len(),
            "observation": observation,
        })
    }
}

pub fn serve_stdio() -> Result<(), BackendError> {
    serve_runtime_stdio(McpRuntime::null())
}

pub fn serve_native_stdio(
    rom_path: impl Into<PathBuf>,
    instructions_per_frame: u64,
) -> Result<(), BackendError> {
    serve_runtime_stdio(McpRuntime::native(rom_path.into(), instructions_per_frame))
}

fn serve_runtime_stdio(runtime: McpRuntime) -> Result<(), BackendError> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_stdio(BufReader::new(stdin.lock()), stdout.lock(), runtime)
}

fn run_stdio<R: BufRead, W: Write>(
    mut reader: R,
    mut writer: W,
    mut runtime: McpRuntime,
) -> Result<(), BackendError> {
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| BackendError::new(format!("failed to read MCP stdin: {error}")))?;
        if bytes == 0 {
            return Ok(());
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = process_message(trimmed, &mut runtime) {
            serde_json::to_writer(&mut writer, &response).map_err(|error| {
                BackendError::new(format!("failed to encode MCP response: {error}"))
            })?;
            writer
                .write_all(b"\n")
                .and_then(|_| writer.flush())
                .map_err(|error| {
                    BackendError::new(format!("failed to write MCP stdout: {error}"))
                })?;
        }
    }
}

fn process_message(line: &str, runtime: &mut McpRuntime) -> Option<Value> {
    let request: Value = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return Some(json_rpc_error(
                Value::Null,
                -32700,
                format!("Parse error: {error}"),
            ));
        }
    };
    let Some(object) = request.as_object() else {
        return Some(json_rpc_error(
            Value::Null,
            -32600,
            "Invalid Request".to_string(),
        ));
    };
    let id = object.get("id").cloned();
    let method = object.get("method").and_then(Value::as_str);
    let is_notification = id.is_none();
    let Some(method) = method else {
        return (!is_notification).then(|| {
            json_rpc_error(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request".to_string(),
            )
        });
    };

    if is_notification {
        return None;
    }
    let id = id.unwrap_or(Value::Null);
    let params = object.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => initialize_result(&params),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => call_tool(&params, runtime),
        _ => Err((-32601, format!("Method not found: {method}"))),
    };

    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json_rpc_error(id, code, message),
    })
}

fn initialize_result(params: &Value) -> Result<Value, (i64, String)> {
    let requested = params
        .as_object()
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(MCP_PROTOCOL_VERSION);
    let protocol_version = if MCP_SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        MCP_PROTOCOL_VERSION
    };
    Ok(json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "bloodyroar2-gym",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Gym-style Bloody Roar 2 control and vision tools"
        },
        "instructions": "Call reset once, then use step or step_buttons. Set screenshot=true for vision observations."
    }))
}

fn call_tool(params: &Value, runtime: &mut McpRuntime) -> Result<Value, (i64, String)> {
    let params = params
        .as_object()
        .ok_or_else(|| (-32602, "tools/call params must be an object".to_string()))?;
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, "tools/call requires a string name".to_string()))?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let arguments = arguments
        .as_object()
        .ok_or_else(|| (-32602, "tool arguments must be an object".to_string()))?;

    let result = match name {
        "action_space" => parse_json_contract(action_space_json()),
        "character_action_space" => parse_json_contract(character_action_space_json()),
        "observation_space" => parse_json_contract(observation_space_json()),
        "health" => Ok(runtime.health()),
        "reset" => {
            let screenshot = optional_bool(arguments, "screenshot", false)?;
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            environment.set_observation_screenshot(screenshot);
            let p1_character = optional_string(arguments, "p1_character")?;
            let p2_character = optional_string(arguments, "p2_character")?;
            let observation = if p1_character.is_some() || p2_character.is_some() {
                let (active_p1, active_p2) = environment.active_characters().ok_or_else(|| {
                    tool_execution_error(BackendError::new(
                        "current backend does not expose active character selection",
                    ))
                })?;
                environment.reset_match(
                    p1_character.unwrap_or(active_p1),
                    p2_character.unwrap_or(active_p2),
                )
            } else {
                environment.reset()
            };
            observation
                .map(|observation| {
                    json!({
                        "observation": observation_value(&observation),
                        "info": parse_info_json(&observation.info_json),
                    })
                })
                .map_err(tool_execution_error)
        }
        "step" => {
            let action_index = required_u64(arguments, "action")?;
            let action_index = usize::try_from(action_index)
                .map_err(|_| (-32602, "action is outside the supported range".to_string()))?;
            let action = Action::from_index(action_index).ok_or_else(|| {
                (
                    -32602,
                    format!(
                        "action must be between 0 and {}",
                        crate::ACTION_SPACE.len() - 1
                    ),
                )
            })?;
            let frames = step_frames(arguments)?;
            let screenshot = optional_bool(arguments, "screenshot", false)?;
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            environment.set_observation_screenshot(screenshot);
            environment
                .step(action, frames)
                .map(|step| step_result_value(&step))
                .map_err(tool_execution_error)
        }
        "step_buttons" => {
            let buttons = arguments
                .get("buttons")
                .ok_or_else(|| (-32602, "step_buttons requires buttons".to_string()))
                .and_then(|buttons| {
                    parse_action_buttons(buttons).map_err(|error| (-32602, error))
                })?;
            let frames = step_frames(arguments)?;
            let screenshot = optional_bool(arguments, "screenshot", false)?;
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            environment.set_observation_screenshot(screenshot);
            environment
                .step_buttons(buttons, frames)
                .map(|step| step_result_value(&step))
                .map_err(tool_execution_error)
        }
        "perform_action" => {
            let character = required_string(arguments, "character")?;
            let player = required_u64(arguments, "player").and_then(|player| {
                Player::from_number(player)
                    .ok_or_else(|| (-32602, "player must be 1 or 2".to_string()))
            })?;
            let action = required_string(arguments, "action")?;
            let facing = arguments
                .get("facing")
                .map(|value| {
                    value
                        .as_str()
                        .and_then(Facing::from_name)
                        .ok_or_else(|| (-32602, "facing must be left or right".to_string()))
                })
                .transpose()?
                .unwrap_or_else(|| player.default_facing());
            let segments = named_action_sequence(character, player, action, facing)
                .map_err(|error| (-32602, error))?;
            let screenshot = optional_bool(arguments, "screenshot", false)?;
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            let character_selection_changed = environment
                .ensure_character_selected(character, player)
                .map_err(tool_execution_error)?;
            environment
                .step_sequence(&segments, screenshot)
                .map(|step| {
                    let mut value = step_result_value(&step);
                    if let Some(object) = value.as_object_mut() {
                        object.insert(
                            "executed_action".to_string(),
                            json!({
                                "character": crate::canonical_character_name(character),
                                "player": player.number(),
                                "action": crate::canonical_named_action(action),
                                "facing": facing.name(),
                                "segments": segments.len(),
                                "character_selection_changed": character_selection_changed,
                            }),
                        );
                    }
                    value
                })
                .map_err(tool_execution_error)
        }
        "step_sequence" => {
            let segments = arguments
                .get("segments")
                .ok_or_else(|| (-32602, "step_sequence requires segments".to_string()))
                .and_then(|segments| {
                    parse_action_segments(segments).map_err(|error| (-32602, error))
                })?;
            let screenshot = optional_bool(arguments, "screenshot", false)?;
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            environment
                .step_sequence(&segments, screenshot)
                .map(|step| step_result_value(&step))
                .map_err(tool_execution_error)
        }
        "screenshot" => {
            let environment = runtime.ensure_environment().map_err(tool_execution_error)?;
            environment.set_observation_screenshot(true);
            let observation = if environment.last_observation().is_some() {
                environment.observe()
            } else {
                environment.reset()
            }
            .map_err(tool_execution_error)?;
            Ok(json!({
                "observation": observation_value(&observation),
                "info": parse_info_json(&observation.info_json),
            }))
        }
        _ => Err((-32602, format!("unknown tool: {name}"))),
    };

    Ok(match result {
        Ok(value) => tool_success(value),
        Err((-32000, message)) => tool_error(message),
        Err(error) => return Err(error),
    })
}

fn parse_json_contract(value: String) -> Result<Value, (i64, String)> {
    serde_json::from_str(&value)
        .map_err(|error| (-32603, format!("failed to encode tool contract: {error}")))
}

fn required_u64(arguments: &Map<String, Value>, name: &str) -> Result<u64, (i64, String)> {
    arguments
        .get(name)
        .and_then(Value::as_u64)
        .ok_or_else(|| (-32602, format!("{name} must be a non-negative integer")))
}

fn required_string<'a>(
    arguments: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, (i64, String)> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| (-32602, format!("{name} must be a string")))
}

fn optional_string<'a>(
    arguments: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, (i64, String)> {
    arguments
        .get(name)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| (-32602, format!("{name} must be a string")))
        })
        .transpose()
}

fn optional_bool(
    arguments: &Map<String, Value>,
    name: &str,
    default: bool,
) -> Result<bool, (i64, String)> {
    arguments
        .get(name)
        .map(|value| {
            value
                .as_bool()
                .ok_or_else(|| (-32602, format!("{name} must be a boolean")))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn step_frames(arguments: &Map<String, Value>) -> Result<u32, (i64, String)> {
    let frames = arguments
        .get("frames")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| (-32602, "frames must be a positive integer".to_string()))
        })
        .transpose()?
        .unwrap_or(1);
    if frames == 0 || frames > MAX_STEP_FRAMES as u64 {
        return Err((
            -32602,
            format!("frames must be between 1 and {MAX_STEP_FRAMES}"),
        ));
    }
    Ok(frames as u32)
}

fn tool_execution_error(error: BackendError) -> (i64, String) {
    (-32000, error.to_string())
}

fn observation_value(observation: &Observation) -> Value {
    serde_json::from_str(&observation.json()).unwrap_or_else(|_| {
        json!({
            "frame": observation.frame,
            "player_health": observation.player_health,
            "opponent_health": observation.opponent_health,
            "beast_meter": observation.beast_meter,
            "opponent_beast_meter": observation.opponent_beast_meter,
            "round_time": observation.round_time,
            "terminal": observation.terminal,
            "native_playable": observation.native_playable,
            "rendered_frame_checksum": observation.rendered_frame_checksum,
            "render_progressing": observation.render_progressing,
            "effects_progressing": observation.effects_progressing,
            "audio_progressing": observation.audio_progressing,
            "emulated_fps": observation.emulated_fps,
            "screenshot_b64": observation.screenshot_b64,
        })
    })
}

fn step_result_value(step: &StepResult) -> Value {
    serde_json::from_str(&step.json()).unwrap_or_else(|_| {
        json!({
            "observation": observation_value(&step.observation),
            "reward": step.reward,
            "terminated": step.terminated,
            "truncated": step.truncated,
            "info": parse_info_json(&step.observation.info_json),
        })
    })
}

fn parse_info_json(info: &str) -> Value {
    serde_json::from_str(info).unwrap_or_else(|_| json!({ "raw": info }))
}

fn tool_success(mut value: Value) -> Value {
    let screenshot = extract_screenshot(&mut value);
    let text = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    let mut content = vec![json!({ "type": "text", "text": text })];
    if let Some(data) = screenshot {
        content.push(json!({
            "type": "image",
            "data": data,
            "mimeType": "image/png",
        }));
    }
    json!({
        "content": content,
        "structuredContent": value,
        "isError": false,
    })
}

fn extract_screenshot(value: &mut Value) -> Option<String> {
    let nested = value
        .as_object()
        .is_some_and(|object| object.contains_key("observation"));
    let data = if nested {
        let screenshot = value
            .as_object_mut()?
            .get_mut("observation")?
            .as_object_mut()?
            .get_mut("screenshot_b64")?;
        let data = screenshot.as_str()?.to_string();
        *screenshot = Value::Null;
        data
    } else {
        let screenshot = value.as_object_mut()?.get_mut("screenshot_b64")?;
        let data = screenshot.as_str()?.to_string();
        *screenshot = Value::Null;
        data
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("screenshot_in_content".to_string(), Value::Bool(true));
    }
    Some(data)
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{ "type": "text", "text": message }],
        "structuredContent": { "error": message },
        "isError": true,
    })
}

fn json_rpc_error(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn tool_definitions() -> Vec<Value> {
    let button_properties = [
        "start", "coin", "service", "up", "down", "left", "right", "punch", "kick", "beast",
        "guard", "p2_start", "p2_coin", "p2_up", "p2_down", "p2_left", "p2_right", "p2_punch",
        "p2_kick", "p2_beast", "p2_guard",
    ]
    .into_iter()
    .map(|name| (name.to_string(), json!({ "type": "boolean" })))
    .collect::<Map<_, _>>();
    let observation_output = json!({
        "type": "object",
        "additionalProperties": true
    });

    vec![
        tool_definition(
            "action_space",
            "Return the complete Discrete(38) P1/P2 action space and button mapping.",
            json!({ "type": "object", "additionalProperties": false }),
            json!({ "type": "object", "additionalProperties": true }),
        ),
        tool_definition(
            "character_action_space",
            "Return all Bloody Roar 2 roster names and release-safe named character actions.",
            json!({ "type": "object", "additionalProperties": false }),
            json!({ "type": "object", "additionalProperties": true }),
        ),
        tool_definition(
            "observation_space",
            "Return the Gym-style observation schema.",
            json!({ "type": "object", "additionalProperties": false }),
            json!({ "type": "object", "additionalProperties": true }),
        ),
        tool_definition(
            "reset",
            "Reset to a cached playable match, optionally selecting actual P1/P2 characters.",
            json!({
                "type": "object",
                "properties": {
                    "p1_character": {
                        "type": "string",
                        "enum": BLOODY_ROAR_2_ROSTER
                    },
                    "p2_character": {
                        "type": "string",
                        "enum": BLOODY_ROAR_2_ROSTER
                    },
                    "screenshot": { "type": "boolean", "default": false }
                },
                "additionalProperties": false
            }),
            observation_output.clone(),
        ),
        tool_definition(
            "step",
            "Apply one discrete P1/P2 action for a bounded number of guest frames.",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": crate::ACTION_SPACE.len() - 1
                    },
                    "frames": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_STEP_FRAMES,
                        "default": 1
                    },
                    "screenshot": { "type": "boolean", "default": false }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
            observation_output.clone(),
        ),
        tool_definition(
            "step_buttons",
            "Apply an arbitrary simultaneous P1/P2 button combination.",
            json!({
                "type": "object",
                "properties": {
                    "buttons": {
                        "type": "object",
                        "properties": button_properties,
                        "additionalProperties": false
                    },
                    "frames": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_STEP_FRAMES,
                        "default": 1
                    },
                    "screenshot": { "type": "boolean", "default": false }
                },
                "required": ["buttons"],
                "additionalProperties": false
            }),
            observation_output.clone(),
        ),
        tool_definition(
            "perform_action",
            "Select the requested character when needed, then execute a release-safe named action for either player.",
            json!({
                "type": "object",
                "properties": {
                    "character": {
                        "type": "string",
                        "enum": BLOODY_ROAR_2_ROSTER
                    },
                    "player": {
                        "type": "integer",
                        "enum": [1, 2]
                    },
                    "action": {
                        "type": "string",
                        "enum": NAMED_ACTIONS.iter().map(|action| action.name).collect::<Vec<_>>()
                    },
                    "facing": {
                        "type": "string",
                        "enum": ["left", "right"]
                    },
                    "screenshot": { "type": "boolean", "default": false }
                },
                "required": ["character", "player", "action"],
                "additionalProperties": false
            }),
            observation_output.clone(),
        ),
        tool_definition(
            "step_sequence",
            "Execute an exact bounded sequence of timed simultaneous P1/P2 button states.",
            json!({
                "type": "object",
                "properties": {
                    "segments": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_ACTION_SEQUENCE_SEGMENTS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "buttons": {
                                    "type": "object",
                                    "properties": button_properties,
                                    "additionalProperties": false
                                },
                                "frames": {
                                    "type": "integer",
                                    "minimum": 1,
                                    "maximum": MAX_STEP_FRAMES
                                }
                            },
                            "required": ["buttons", "frames"],
                            "additionalProperties": false
                        }
                    },
                    "screenshot": { "type": "boolean", "default": false }
                },
                "required": ["segments"],
                "additionalProperties": false
            }),
            observation_output.clone(),
        ),
        tool_definition(
            "screenshot",
            "Capture the current 640x480 GUI-equivalent frame without advancing emulation.",
            json!({ "type": "object", "additionalProperties": false }),
            observation_output.clone(),
        ),
        tool_definition(
            "health",
            "Report MCP/native backend readiness and the latest observation if loaded.",
            json!({ "type": "object", "additionalProperties": false }),
            observation_output,
        ),
    ]
}

fn tool_definition(
    name: &str,
    description: &str,
    input_schema: Value,
    output_schema: Value,
) -> Value {
    json!({
        "name": name,
        "title": name.replace('_', " "),
        "description": description,
        "inputSchema": input_schema,
        "outputSchema": output_schema,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use serde_json::Value;

    use super::{MCP_PROTOCOL_VERSION, McpRuntime, process_message, run_stdio};

    fn request(id: u64, method: &str, params: Value) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })
        .to_string()
    }

    #[test]
    fn stdio_server_initializes_lists_tools_and_steps_both_players() {
        let messages = [
            request(
                1,
                "initialize",
                serde_json::json!({ "protocolVersion": MCP_PROTOCOL_VERSION }),
            ),
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })
            .to_string(),
            request(2, "tools/list", serde_json::json!({})),
            request(
                3,
                "tools/call",
                serde_json::json!({
                    "name": "reset",
                    "arguments": { "screenshot": false }
                }),
            ),
            request(
                4,
                "tools/call",
                serde_json::json!({
                    "name": "step",
                    "arguments": { "action": 5, "frames": 2 }
                }),
            ),
            request(
                5,
                "tools/call",
                serde_json::json!({
                    "name": "step_buttons",
                    "arguments": {
                        "buttons": {
                            "punch": true,
                            "p2_punch": true,
                            "p2_guard": true
                        },
                        "frames": 3
                    }
                }),
            ),
            request(
                6,
                "tools/call",
                serde_json::json!({
                    "name": "character_action_space",
                    "arguments": {}
                }),
            ),
            request(
                7,
                "tools/call",
                serde_json::json!({
                    "name": "perform_action",
                    "arguments": {
                        "character": "long",
                        "player": 2,
                        "action": "forward_punch",
                        "facing": "left"
                    }
                }),
            ),
            request(
                8,
                "tools/call",
                serde_json::json!({
                    "name": "step_sequence",
                    "arguments": {
                        "segments": [
                            {
                                "buttons": {"punch": true, "p2_kick": true},
                                "frames": 2
                            },
                            {
                                "buttons": {},
                                "frames": 2
                            }
                        ]
                    }
                }),
            ),
            request(
                9,
                "tools/call",
                serde_json::json!({ "name": "health", "arguments": {} }),
            ),
        ]
        .join("\n");
        let mut output = Vec::new();

        run_stdio(
            BufReader::new(Cursor::new(format!("{messages}\n"))),
            &mut output,
            McpRuntime::null(),
        )
        .expect("MCP stdio run");

        let responses = String::from_utf8(output)
            .expect("UTF-8")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("JSON response"))
            .collect::<Vec<_>>();
        assert_eq!(
            responses.len(),
            9,
            "notifications must not receive responses"
        );
        assert_eq!(
            responses[0]["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(
            responses[1]["result"]["tools"].as_array().unwrap().len(),
            10
        );
        assert_eq!(
            responses[3]["result"]["structuredContent"]["observation"]["frame"],
            2
        );
        assert_eq!(
            responses[4]["result"]["structuredContent"]["observation"]["frame"],
            5
        );
        assert_eq!(
            responses[5]["result"]["structuredContent"]["roster"][4],
            "long"
        );
        assert_eq!(
            responses[6]["result"]["structuredContent"]["executed_action"]["player"],
            2
        );
        assert_eq!(
            responses[6]["result"]["structuredContent"]["observation"]["frame"], 7,
            "selecting a different character resets the match before executing the action"
        );
        assert_eq!(
            responses[6]["result"]["structuredContent"]["observation"]["effects_progressing"],
            true
        );
        assert_eq!(
            responses[6]["result"]["structuredContent"]["info"]["sequence"]["released"],
            true
        );
        assert_eq!(
            responses[7]["result"]["structuredContent"]["observation"]["frame"],
            11
        );
        assert_eq!(
            responses[8]["result"]["structuredContent"]["backend"],
            "null"
        );
        assert_eq!(responses[8]["result"]["structuredContent"]["loaded"], true);
    }

    #[test]
    fn native_health_does_not_boot_rom_until_gameplay_tool_is_called() {
        let mut runtime = McpRuntime::native("/missing/roms".into(), 500_000);
        let response = process_message(
            &request(
                1,
                "tools/call",
                serde_json::json!({ "name": "health", "arguments": {} }),
            ),
            &mut runtime,
        )
        .expect("response");

        assert_eq!(response["result"]["structuredContent"]["backend"], "native");
        assert_eq!(response["result"]["structuredContent"]["loaded"], false);
    }

    #[test]
    fn invalid_tool_arguments_return_protocol_errors_without_panicking() {
        let mut runtime = McpRuntime::null();
        let invalid_frames = process_message(
            &request(
                1,
                "tools/call",
                serde_json::json!({
                    "name": "step",
                    "arguments": { "action": 5, "frames": 0 }
                }),
            ),
            &mut runtime,
        )
        .expect("response");
        assert_eq!(invalid_frames["error"]["code"], -32602);

        let unknown = process_message(
            &request(
                2,
                "tools/call",
                serde_json::json!({ "name": "missing", "arguments": {} }),
            ),
            &mut runtime,
        )
        .expect("response");
        assert_eq!(unknown["error"]["code"], -32602);
    }
}
