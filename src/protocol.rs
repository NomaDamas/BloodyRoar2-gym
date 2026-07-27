use crate::ACTION_SPACE;

pub fn api_index_json() -> String {
    format!(
        "{{\"name\":\"bloodyroar2-gym\",\"version\":\"{}\",\"endpoints\":[\"GET /\",\"GET /action_space\",\"GET /observation_space\",\"POST /reset\",\"POST /step\"],\"request_options\":{{\"step_control\":\"provide exactly one of action or buttons\",\"action\":\"backward-compatible discrete action index\",\"buttons\":\"arbitrary simultaneous boolean controls\",\"frames\":\"integer from 1 through {}\",\"screenshot\":\"optional boolean; defaults false\"}},\"native_defaults\":{{\"reset\":\"playable match checkpoint\",\"screenshot\":\"640x480 GUI-equivalent PNG\"}},\"asset_policy\":\"No ROMs, BIOS files, or proprietary game binaries are included. Provide legally obtained assets at runtime.\"}}",
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

pub fn observation_space_json() -> String {
    "{\"type\":\"Dict\",\"fields\":{\"frame\":{\"type\":\"u64\"},\"player_health\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"opponent_health\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"beast_meter\":{\"type\":\"Box\",\"low\":0.0,\"high\":1.0},\"round_time\":{\"type\":\"Box\",\"low\":0.0,\"high\":99.0},\"terminal\":{\"type\":\"bool\"},\"screenshot_b64\":{\"type\":\"optional_base64_png\"}}}".to_string()
}
