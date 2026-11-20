//! Adapter for a caller-built BloodyRoar2Recomp executable.
//!
//! The recompiled executable is deliberately kept outside this repository.  A
//! [`RecompBackend`] starts a caller-owned executable, connects to its local
//! JSON-over-newline debug server, injects one PS1 pad state, and waits for the
//! requested number of guest frames.  The adapter does not inspect, copy, or
//! package the disc image or generated game C.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::action::ActionButtons;
use crate::backend::{Backend, BackendError, Observation};

pub const DEFAULT_RECOMP_DEBUG_PORT: u16 = 4370;
const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(3);
const DEFAULT_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_FRAME_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(4);
static REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static SCREENSHOT_ID: AtomicU64 = AtomicU64::new(1);

/// Launch and connection settings for a local Recomp runtime.
#[derive(Clone, Debug)]
pub struct RecompConfig {
    /// Path to the caller-built `BloodyRoar2_Recompiled` executable.
    pub executable: PathBuf,
    /// Path to a legally supplied `.cue`, `.bin`, `.iso`, or `.chd` image.
    pub disc: PathBuf,
    /// Local debug TCP port compiled/configured into the runtime.
    pub debug_port: u16,
    /// How long to wait for the runtime's debug server during startup.
    pub startup_timeout: Duration,
    /// Timeout for one JSON request/response exchange.
    pub command_timeout: Duration,
    /// Maximum wall time allowed while waiting for a guest frame target.
    pub frame_timeout: Duration,
}

impl RecompConfig {
    pub fn new(executable: impl Into<PathBuf>, disc: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            disc: disc.into(),
            debug_port: DEFAULT_RECOMP_DEBUG_PORT,
            startup_timeout: DEFAULT_STARTUP_TIMEOUT,
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
            frame_timeout: DEFAULT_FRAME_TIMEOUT,
        }
    }

    fn validate(&self) -> Result<(), BackendError> {
        if !self.executable.is_file() {
            return Err(BackendError::new(format!(
                "Recomp executable is not a regular file: {}",
                self.executable.display()
            )));
        }
        if !self.disc.is_file() {
            return Err(BackendError::new(format!(
                "caller-supplied disc image is not a regular file: {}",
                self.disc.display()
            )));
        }
        let extension = self
            .disc
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(extension.as_str(), "bin" | "cue" | "iso" | "chd") {
            return Err(BackendError::new(format!(
                "unsupported disc image extension '.{extension}'; use .bin, .cue, .iso, or .chd"
            )));
        }
        if self.debug_port == 0 {
            return Err(BackendError::new(
                "Recomp debug port must be between 1 and 65535",
            ));
        }
        if self.startup_timeout.is_zero()
            || self.command_timeout.is_zero()
            || self.frame_timeout.is_zero()
        {
            return Err(BackendError::new(
                "Recomp startup, command, and frame timeouts must be positive",
            ));
        }
        Ok(())
    }
}

/// One-command-per-connection client for `psxrecomp/runtime/src/debug_server.c`.
#[derive(Clone, Debug)]
pub struct RecompDebugClient {
    address: SocketAddr,
    command_timeout: Duration,
}

impl RecompDebugClient {
    pub fn localhost(port: u16, command_timeout: Duration) -> Result<Self, BackendError> {
        if port == 0 {
            return Err(BackendError::new(
                "Recomp debug port must be between 1 and 65535",
            ));
        }
        let address = ("127.0.0.1", port)
            .to_socket_addrs()
            .map_err(|error| BackendError::new(format!("resolve Recomp debug address: {error}")))?
            .next()
            .ok_or_else(|| BackendError::new("Recomp debug address resolved to no endpoints"))?;
        Ok(Self {
            address,
            command_timeout,
        })
    }

    pub fn command(&self, command: &str, params: Value) -> Result<Value, BackendError> {
        let mut request = Map::new();
        request.insert(
            "id".to_string(),
            Value::Number(REQUEST_ID.fetch_add(1, Ordering::Relaxed).into()),
        );
        request.insert("cmd".to_string(), Value::String(command.to_string()));
        if let Some(params) = params.as_object() {
            for (key, value) in params {
                request.insert(key.clone(), value.clone());
            }
        } else if !params.is_null() {
            return Err(BackendError::new(format!(
                "Recomp command parameters for '{command}' must be a JSON object"
            )));
        }

        let mut stream =
            TcpStream::connect_timeout(&self.address, self.command_timeout).map_err(|error| {
                BackendError::new(format!(
                    "cannot connect to Recomp debug server at {}: {error}",
                    self.address
                ))
            })?;
        stream
            .set_read_timeout(Some(self.command_timeout))
            .and_then(|_| stream.set_write_timeout(Some(self.command_timeout)))
            .map_err(|error| {
                BackendError::new(format!("configure Recomp debug socket: {error}"))
            })?;
        let encoded = serde_json::to_vec(&Value::Object(request))
            .map_err(|error| BackendError::new(format!("encode Recomp command: {error}")))?;
        stream
            .write_all(&encoded)
            .and_then(|_| stream.write_all(b"\n"))
            .map_err(|error| {
                BackendError::new(format!("send Recomp command '{command}': {error}"))
            })?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .map_err(|error| {
                BackendError::new(format!("read Recomp response for '{command}': {error}"))
            })?;
        if line.trim().is_empty() {
            return Err(BackendError::new(format!(
                "Recomp debug server closed without a response to '{command}'"
            )));
        }
        let response: Value = serde_json::from_str(line.trim()).map_err(|error| {
            BackendError::new(format!("invalid Recomp response for '{command}': {error}"))
        })?;
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            let message = response
                .get("error")
                .and_then(Value::as_str)
                .or_else(|| response.get("err").and_then(Value::as_str))
                .unwrap_or("unknown Recomp debug-server error");
            return Err(BackendError::new(format!(
                "Recomp '{command}' failed: {message}"
            )));
        }
        Ok(response)
    }

    pub fn frame(&self) -> Result<u64, BackendError> {
        self.command("frame", Value::Object(Map::new()))?
            .get("frame")
            .and_then(Value::as_u64)
            .ok_or_else(|| BackendError::new("Recomp frame response did not contain a u64 frame"))
    }
}

/// Gym backend that drives a caller-owned Recomp executable over its debug port.
pub struct RecompBackend {
    config: RecompConfig,
    client: RecompDebugClient,
    child: Option<Child>,
    frame: u64,
    screenshot_enabled: bool,
    last_input_mask: u16,
}

impl RecompBackend {
    pub fn new(config: RecompConfig) -> Result<Self, BackendError> {
        config.validate()?;
        let client = RecompDebugClient::localhost(config.debug_port, config.command_timeout)?;
        let mut backend = Self {
            config,
            client,
            child: None,
            frame: 0,
            screenshot_enabled: false,
            last_input_mask: 0xffff,
        };
        backend.start_runtime()?;
        Ok(backend)
    }

    pub fn from_paths(
        executable: impl Into<PathBuf>,
        disc: impl Into<PathBuf>,
        debug_port: u16,
    ) -> Result<Self, BackendError> {
        let mut config = RecompConfig::new(executable, disc);
        config.debug_port = debug_port;
        Self::new(config)
    }

    pub fn config(&self) -> &RecompConfig {
        &self.config
    }

    fn start_runtime(&mut self) -> Result<(), BackendError> {
        let mut command = Command::new(&self.config.executable);
        command
            .arg("--headless")
            .arg("--no-launcher")
            .arg("--debug-port")
            .arg(self.config.debug_port.to_string())
            .arg("--disc")
            .arg(&self.config.disc)
            .env("PSX_HEADLESS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            // Keep runtime diagnostics on the parent stderr.  MCP responses
            // stay on this process's stdout, while launch/boot errors remain
            // visible to a macOS terminal or service supervisor.
            .stderr(Stdio::inherit());
        let child = command.spawn().map_err(|error| {
            BackendError::new(format!(
                "failed to launch caller-owned Recomp executable {}: {error}",
                self.config.executable.display()
            ))
        })?;
        self.child = Some(child);

        let deadline = Instant::now() + self.config.startup_timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            if let Some(child) = self.child.as_mut()
                && let Some(status) = child.try_wait().map_err(|error| {
                    BackendError::new(format!("inspect Recomp process status: {error}"))
                })?
            {
                self.stop_runtime();
                return Err(BackendError::new(format!(
                    "Recomp executable exited before its debug server became ready ({status})"
                )));
            }
            match self.client.command("ping", Value::Object(Map::new())) {
                Ok(response) => {
                    self.frame = response.get("frame").and_then(Value::as_u64).unwrap_or(0);
                    self.last_input_mask = 0xffff;
                    return Ok(());
                }
                Err(error) => last_error = Some(error.to_string()),
            }
            thread::sleep(POLL_INTERVAL);
        }
        self.stop_runtime();
        Err(BackendError::new(format!(
            "Recomp debug server at 127.0.0.1:{} did not become ready within {:?}: {}",
            self.config.debug_port,
            self.config.startup_timeout,
            last_error.unwrap_or_else(|| "no response".to_string())
        )))
    }

    fn stop_runtime(&mut self) {
        if let Some(mut child) = self.child.take() {
            // The port may be occupied by another runtime. Never send the
            // upstream `quit` command based only on a successful ping: its
            // handler calls exit(0), so a port collision could terminate a
            // process this adapter does not own. Kill and reap only our child.
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn restart_runtime(&mut self) -> Result<(), BackendError> {
        self.stop_runtime();
        self.start_runtime()
    }

    fn current_frame(&mut self) -> Result<u64, BackendError> {
        let frame = self.client.frame()?;
        self.frame = frame;
        Ok(frame)
    }

    fn wait_for_frame(&mut self, target: u64) -> Result<u64, BackendError> {
        let deadline = Instant::now() + self.config.frame_timeout;
        loop {
            let frame = self.current_frame()?;
            if frame >= target {
                return Ok(frame);
            }
            if Instant::now() >= deadline {
                return Err(BackendError::new(format!(
                    "Recomp frame did not reach {target} within {:?} (last frame {frame})",
                    self.config.frame_timeout
                )));
            }
            thread::sleep(POLL_INTERVAL);
        }
    }

    fn capture_screenshot(&self) -> Result<String, BackendError> {
        let id = SCREENSHOT_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bloodyroar2-recomp-{}-{id}.png",
            std::process::id()
        ));
        let response = self
            .client
            .command("screenshot", json!({"path": path.to_string_lossy()}))?;
        let reported_path = response
            .get("path")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| path.clone());
        if reported_path != path {
            return Err(BackendError::new(format!(
                "Recomp screenshot path differed from the requested temporary path: {}",
                reported_path.display()
            )));
        }
        let bytes = fs::read(&path).map_err(|error| {
            BackendError::new(format!(
                "Recomp screenshot command succeeded but PNG could not be read at {}: {error}",
                path.display()
            ))
        });
        let _ = fs::remove_file(&path);
        let bytes = bytes?;
        Ok(base64_encode(&bytes))
    }

    fn observation(&mut self, frame: u64) -> Result<Observation, BackendError> {
        let screenshot_b64 = if self.screenshot_enabled {
            Some(self.capture_screenshot()?)
        } else {
            None
        };
        let checksum = screenshot_b64
            .as_deref()
            .map(fnv1a_base64)
            .unwrap_or(frame as u32);
        let info_json = json!({
            "backend": "psx-recomp",
            "executable": self.config.executable,
            "disc": self.config.disc,
            "debug_server": format!("127.0.0.1:{}", self.config.debug_port),
            "health_source": "unavailable_from_recomp_debug_protocol",
            "player_two_control": "unsupported_by_set_input_pad1_protocol",
            "pad1_buttons": format!("0x{:04X}", self.last_input_mask),
            "caller_supplied_assets": true,
        })
        .to_string();
        Ok(Observation {
            frame,
            player_health: 1.0,
            opponent_health: 1.0,
            beast_meter: 0.0,
            opponent_beast_meter: 0.0,
            round_time: (99.0 - frame as f32 / 60.0).max(0.0),
            terminal: false,
            native_playable: frame > 0,
            rendered_frame_checksum: checksum,
            render_progressing: frame > 0,
            effects_progressing: self.last_input_mask != 0xffff,
            audio_progressing: false,
            emulated_fps: 60.0,
            screenshot_b64,
            info_json,
        })
    }
}

impl Backend for RecompBackend {
    fn set_observation_screenshot(&mut self, enabled: bool) {
        self.screenshot_enabled = enabled;
    }

    fn diagnostic_json(&self) -> Option<String> {
        Some(
            json!({
                "backend": "psx-recomp",
                "executable": self.config.executable,
                "disc": self.config.disc,
                "debug_port": self.config.debug_port,
                "frame": self.frame,
                "screenshot_enabled": self.screenshot_enabled,
                "player_two_control": "unsupported_by_set_input_pad1_protocol",
            })
            .to_string(),
        )
    }

    fn reset(&mut self) -> Result<Observation, BackendError> {
        self.restart_runtime()?;
        self.last_input_mask = 0xffff;
        let frame = self.current_frame()?;
        self.observation(frame)
    }

    fn step(&mut self, buttons: ActionButtons, frames: u32) -> Result<Observation, BackendError> {
        if frames == 0 {
            return Err(BackendError::new("Recomp step frames must be positive"));
        }
        if has_player_two_input(buttons) {
            return Err(BackendError::new(
                "BloodyRoar2Recomp's set_input protocol exposes pad1 only and has no arcade coin/service inputs; use P1 pad controls or the Rust native backend for P2/arcade controls",
            ));
        }
        let before = self.current_frame()?;
        let mask = action_buttons_to_pad_word(buttons);
        self.last_input_mask = mask;
        self.client
            .command("set_input", json!({"buttons": format!("0x{mask:04X}")}))?;
        let waited = self.wait_for_frame(before.saturating_add(frames as u64));
        let clear = self
            .client
            .command("clear_input", Value::Object(Map::new()));
        self.last_input_mask = 0xffff;
        if let Err(error) = waited {
            let _ = clear;
            return Err(error);
        }
        clear?;
        self.observation(self.frame)
    }

    fn observe(&mut self) -> Result<Observation, BackendError> {
        let frame = self.current_frame()?;
        self.observation(frame)
    }
}

impl Drop for RecompBackend {
    fn drop(&mut self) {
        self.stop_runtime();
    }
}

fn has_player_two_input(buttons: ActionButtons) -> bool {
    buttons.coin
        || buttons.service
        || buttons.p2_start
        || buttons.p2_coin
        || buttons.p2_up
        || buttons.p2_down
        || buttons.p2_left
        || buttons.p2_right
        || buttons.p2_punch
        || buttons.p2_kick
        || buttons.p2_beast
        || buttons.p2_guard
}

/// Convert Gym controls to the active-low PS1 digital pad word used by Recomp.
pub fn action_buttons_to_pad_word(buttons: ActionButtons) -> u16 {
    let mut word = 0xffff;
    set_pressed(&mut word, 1 << 3, buttons.start);
    set_pressed(&mut word, 1 << 4, buttons.up);
    set_pressed(&mut word, 1 << 5, buttons.right);
    set_pressed(&mut word, 1 << 6, buttons.down);
    set_pressed(&mut word, 1 << 7, buttons.left);
    // The stock Recomp keyboard map uses square/cross/circle/triangle for the
    // four Bloody Roar controls (punch/kick/beast/guard respectively).
    set_pressed(&mut word, 1 << 15, buttons.punch);
    set_pressed(&mut word, 1 << 14, buttons.kick);
    set_pressed(&mut word, 1 << 13, buttons.beast);
    set_pressed(&mut word, 1 << 12, buttons.guard);
    word
}

fn set_pressed(word: &mut u16, bit: u16, pressed: bool) {
    if pressed {
        *word &= !bit;
    }
}

fn fnv1a_base64(value: &str) -> u32 {
    // The checksum is only a stable observation hint; hashing the encoded
    // bytes keeps the observation independent from a temporary file path.
    value.bytes().fold(0x811c9dc5, |hash, byte| {
        hash.wrapping_mul(0x01000193) ^ u32::from(byte)
    })
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(a >> 2) as usize] as char);
        output.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

/// Build-only command plan for the caller-owned Recomp checkout.
pub fn recomp_build_plan_json(
    project_root: &Path,
    disc: &Path,
    build_dir: &Path,
    config: &str,
    target: &str,
) -> String {
    let (exe_basename, cmake_extra, regional_inputs) = match target {
        "psx-runtime-us" => (
            "BloodyRoar2_Recompiled_USA",
            vec!["-DPSX_DEBUG_TOOLS=ON"],
            Vec::<Value>::new(),
        ),
        "psx-runtime-japan" => (
            "BloodyRoar2_Recompiled_Japan",
            vec!["-DPSX_DEBUG_TOOLS=ON", "-DBR2_BUILD_JAPAN=ON"],
            Vec::<Value>::new(),
        ),
        "psx-runtime-universal" => (
            "BloodyRoar2_Recompiled",
            vec!["-DPSX_DEBUG_TOOLS=ON", "-DBR2_BUILD_UNIVERSAL=ON"],
            vec![
                json!({"config": "game_us.toml", "disc": "<usa-disc>"}),
                json!({"config": "game_japan.toml", "disc": "<japan-disc>"}),
            ],
        ),
        _ => (
            "BloodyRoar2_Recompiled",
            vec!["-DPSX_DEBUG_TOOLS=ON"],
            Vec::<Value>::new(),
        ),
    };
    let mut commands = vec![
        json!(["git", "submodule", "update", "--init", "--recursive"]),
        json!(["./psxrecomp/tools/ci/build_emitters.sh"]),
        json!([
            "python3",
            "psxrecomp/psxrecomp_cli.py",
            "verify-disc",
            "--config",
            config,
            "--project-root",
            project_root,
            "--disc",
            disc
        ]),
        json!([
            "python3",
            "psxrecomp/psxrecomp_cli.py",
            "generate",
            "--config",
            config,
            "--project-root",
            project_root,
            "--disc",
            disc
        ]),
    ];
    if target == "psx-runtime-universal" {
        commands.extend([
            json!([
                "python3",
                "psxrecomp/psxrecomp_cli.py",
                "verify-disc",
                "--config",
                project_root.join("game_us.toml"),
                "--project-root",
                project_root,
                "--disc",
                "<usa-disc>"
            ]),
            json!([
                "python3",
                "psxrecomp/psxrecomp_cli.py",
                "generate",
                "--config",
                project_root.join("game_us.toml"),
                "--project-root",
                project_root,
                "--disc",
                "<usa-disc>"
            ]),
            json!([
                "python3",
                "psxrecomp/psxrecomp_cli.py",
                "verify-disc",
                "--config",
                project_root.join("game_japan.toml"),
                "--project-root",
                project_root,
                "--disc",
                "<japan-disc>"
            ]),
            json!([
                "python3",
                "psxrecomp/psxrecomp_cli.py",
                "generate",
                "--config",
                project_root.join("game_japan.toml"),
                "--project-root",
                project_root,
                "--disc",
                "<japan-disc>"
            ]),
        ]);
    }
    let mut rebuild = vec![
        json!("python3"),
        json!("psxrecomp/psxrecomp_cli.py"),
        json!("rebuild"),
        json!("--config"),
        json!(config),
        json!("--project-root"),
        json!(project_root),
        json!("--build-dir"),
        json!(build_dir),
        json!("--target"),
        json!(target),
        json!("--exe-basename"),
        json!(exe_basename),
        json!("--no-pgo"),
    ];
    for option in &cmake_extra {
        rebuild.push(json!(format!("--cmake-extra={option}")));
    }
    commands.push(Value::Array(rebuild));
    json!({
        "schema": "bloodyroar2.recomp-build-plan/v1",
        "project_root": project_root,
        "disc": disc,
        "build_dir": build_dir,
        "config": config,
        "target": target,
        "caller_owned_inputs": ["disc image", "generated game C", "recompiled executable"],
        "additional_disc_inputs": regional_inputs,
        "tracked_outputs": false,
        "cmake_extra": cmake_extra,
        "commands": commands,
        "runtime": {
            "launch": ["<recompiled-executable>", "--headless", "--no-launcher", "--debug-port", "4370", "--disc", disc],
            "mcp": ["bloodyroar2-gym", "recomp-mcp", "<recompiled-executable>", disc, "4370"],
            "debug_tools": "PSX_DEBUG_TOOLS=ON",
            "exe_basename": exe_basename,
            "regional_generation_required": target == "psx-runtime-universal"
        }
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        RecompDebugClient, action_buttons_to_pad_word, base64_encode, recomp_build_plan_json,
    };
    use crate::action::ActionButtons;
    use serde_json::Value;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn maps_controls_to_active_low_ps1_word() {
        let word = action_buttons_to_pad_word(ActionButtons {
            start: true,
            up: true,
            punch: true,
            kick: true,
            beast: true,
            guard: true,
            ..ActionButtons::default()
        });
        assert_eq!(word & (1 << 3), 0);
        assert_eq!(word & (1 << 4), 0);
        assert_eq!(word & (1 << 15), 0);
        assert_eq!(word & (1 << 14), 0);
        assert_eq!(word & (1 << 13), 0);
        assert_eq!(word & (1 << 12), 0);
        assert_eq!(word & (1 << 7), 1 << 7);
    }

    #[test]
    fn base64_encoder_handles_padding() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
    }

    #[test]
    fn debug_client_speaks_one_request_per_connection() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener");
        let port = listener.local_addr().expect("address").port();
        let server = thread::spawn(move || {
            for expected_frame in [7_u64, 8_u64] {
                let (stream, _) = listener.accept().expect("connection");
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut line = String::new();
                reader.read_line(&mut line).expect("request");
                let request: Value = serde_json::from_str(line.trim()).expect("JSON request");
                assert_eq!(request["cmd"], "frame");
                let mut stream = stream;
                writeln!(stream, r#"{{"id":1,"ok":true,"frame":{expected_frame}}}"#)
                    .expect("response");
            }
        });
        let client = RecompDebugClient::localhost(port, Duration::from_secs(2)).expect("client");
        assert_eq!(client.frame().expect("first frame"), 7);
        assert_eq!(client.frame().expect("second frame"), 8);
        server.join().expect("server");
    }

    #[test]
    fn build_plan_keeps_disc_and_generated_outputs_caller_owned() {
        let plan: Value = serde_json::from_str(&recomp_build_plan_json(
            Path::new("/tmp/recomp"),
            Path::new("/private/disc/game.cue"),
            Path::new("/tmp/recomp/build-release"),
            "game.toml",
            "psx-runtime",
        ))
        .expect("plan JSON");
        assert_eq!(plan["tracked_outputs"], false);
        assert_eq!(plan["disc"], "/private/disc/game.cue");
        assert!(plan["commands"].as_array().expect("commands").len() >= 5);
        let rebuild = plan["commands"]
            .as_array()
            .expect("commands")
            .last()
            .expect("rebuild command");
        assert!(
            rebuild
                .as_array()
                .expect("rebuild argv")
                .iter()
                .any(|arg| arg == "--cmake-extra=-DPSX_DEBUG_TOOLS=ON")
        );
    }

    #[test]
    fn build_plan_marks_japan_and_universal_cmake_requirements() {
        let japan: Value = serde_json::from_str(&recomp_build_plan_json(
            Path::new("/tmp/recomp"),
            Path::new("/private/disc/japan.cue"),
            Path::new("/tmp/recomp/build-japan"),
            "game_japan.toml",
            "psx-runtime-japan",
        ))
        .expect("Japan plan JSON");
        assert_eq!(
            japan["runtime"]["exe_basename"],
            "BloodyRoar2_Recompiled_Japan"
        );
        assert!(
            japan["cmake_extra"]
                .as_array()
                .expect("Japan CMake options")
                .iter()
                .any(|option| option == "-DBR2_BUILD_JAPAN=ON")
        );

        let universal: Value = serde_json::from_str(&recomp_build_plan_json(
            Path::new("/tmp/recomp"),
            Path::new("/private/disc/eu.cue"),
            Path::new("/tmp/recomp/build-universal"),
            "game.toml",
            "psx-runtime-universal",
        ))
        .expect("universal plan JSON");
        assert_eq!(universal["runtime"]["regional_generation_required"], true);
        assert_eq!(
            universal["additional_disc_inputs"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let commands = universal["commands"]
            .as_array()
            .expect("universal commands");
        let rebuild = commands.last().expect("universal rebuild");
        assert!(
            rebuild
                .as_array()
                .expect("universal rebuild argv")
                .iter()
                .any(|arg| arg == "--cmake-extra=-DBR2_BUILD_UNIVERSAL=ON")
        );
    }
}
