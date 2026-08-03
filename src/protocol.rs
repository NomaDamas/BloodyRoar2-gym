use serde_json::json;

use crate::{ACTION_SPACE, BLOODY_ROAR_2_ROSTER, NAMED_ACTIONS};

pub fn api_index_json() -> String {
    format!(
        "{{\"name\":\"bloodyroar2-gym\",\"version\":\"{}\",\"endpoints\":[\"GET /\",\"GET /action_space\",\"GET /character_action_space\",\"GET /observation_space\",\"GET /health\",\"GET /screenshot\",\"POST /reset\",\"POST /step\",\"POST /action\",\"POST /step_sequence\"],\"mcp_commands\":[\"mcp\",\"native-mcp\"],\"request_options\":{{\"step_control\":\"provide exactly one of action or buttons\",\"action\":\"backward-compatible discrete action index\",\"buttons\":\"arbitrary simultaneous boolean controls\",\"frames\":\"integer from 1 through {}\",\"screenshot\":\"optional boolean; defaults false\",\"named_action\":\"character, player, action and optional facing\",\"step_sequence\":\"bounded array of timed button segments\"}},\"native_defaults\":{{\"reset\":\"playable match checkpoint\",\"screenshot\":\"640x480 GUI-equivalent PNG\"}},\"asset_policy\":\"No ROMs, BIOS files, or proprietary game binaries are included. Provide legally obtained assets at runtime.\"}}",
        env!("CARGO_PKG_VERSION"),
        crate::env::MAX_STEP_FRAMES,
    )
}

pub fn action_space_json() -> String {
    let actions = ACTION_SPACE
        .iter()
        .map(|action| {
            format!(
                "{{\"index\":{},\"name\":\"{}\",\"buttons\":{}}}",
                action.index(),
                action.name(),
                action.buttons().json()
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        "{{\"type\":\"Discrete\",\"n\":{},\"actions\":[{}]}}",
        ACTION_SPACE.len(),
        actions
    )
}

pub fn character_action_space_json() -> String {
    let actions = NAMED_ACTIONS
        .iter()
        .map(|action| {
            json!({
                "name": action.name,
                "description": action.description,
                "release_safe": true,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "type": "CharacterActionSpace",
        "roster": BLOODY_ROAR_2_ROSTER,
        "players": [1, 2],
        "facing": ["left", "right"],
        "named_actions": actions,
        "semantics": "Named actions are release-safe controller macros available to every roster character. Exact character-specific command lists can be expressed through step_sequence.",
        "arbitrary_sequence": {
            "endpoint": "POST /step_sequence",
            "mcp_tool": "step_sequence",
            "max_segments": crate::MAX_ACTION_SEQUENCE_SEGMENTS,
            "max_total_frames": crate::MAX_STEP_FRAMES,
        }
    })
    .to_string()
}

pub fn observation_space_json() -> String {
    "{\"type\":\"Dict\",\"fields\":{\"frame\":{\"type\":\"u64\"},\"player_health\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"opponent_health\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"beast_meter\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"opponent_beast_meter\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"round_time\":{\"type\":\"Box\",\"low\":0.0,\"high\":99.0},\"terminal\":{\"type\":\"bool\"},\"native_playable\":{\"type\":\"bool\"},\"rendered_frame_checksum\":{\"type\":\"u32\"},\"render_progressing\":{\"type\":\"bool\"},\"effects_progressing\":{\"type\":\"bool\"},\"audio_progressing\":{\"type\":\"bool\"},\"emulated_fps\":{\"type\":\"f32\",\"low\":0.0},\"screenshot_b64\":{\"type\":\"optional_base64_png\"}}}".to_string()
}
