use crate::ACTION_SPACE;

pub fn api_index_json() -> String {
    format!(
        "{{\"name\":\"bloodyroar2-gym\",\"version\":\"{}\",\"endpoints\":[\"GET /\",\"GET /action_space\",\"GET /observation_space\",\"POST /reset\",\"POST /step\"],\"asset_policy\":\"No ROMs, BIOS files, or proprietary game binaries are included. Provide legally obtained assets at runtime.\"}}",
        env!("CARGO_PKG_VERSION")
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
