pub mod action;
pub mod backend;
pub mod env;
pub mod mame;
pub mod mcp;
pub mod moves;
pub mod native;
pub mod protocol;
pub mod server;
pub mod zinc;

pub use action::{ACTION_SPACE, Action, ActionButtons};
pub use backend::{Backend, BackendError, NullBackend};
pub use env::{BloodyRoar2Env, MAX_STEP_FRAMES, StepResult};
pub use mame::{MameConfig, MameRuntime};
pub use mcp::{serve_native_stdio as serve_native_mcp_stdio, serve_stdio as serve_mcp_stdio};
pub use moves::{
    ActionSegment, BLOODY_ROAR_2_ROSTER, Facing, MAX_ACTION_SEQUENCE_SEGMENTS, NAMED_ACTIONS,
    NamedActionSpec, Player, canonical_character_name, canonical_named_action,
    named_action_sequence,
};
pub use native::{
    GenericNativePlatform, NativeBackend, NativeDisplayFrame, NativeEmulator,
    NativeGpuDisplayCandidate, NativeGpuDrawCapturePredicate, NativeInputActivity,
    NativePlatformInfo, NativePlayableSegment, NativePlayableStartup, NativeRomAssetExpectation,
    NativeRomAssetMatch, NativeRomAssetMismatch, NativeRomCacheReport, NativeRomCachedScan,
    NativeRomCompatibilityReport, NativeRomDuplicateAsset, NativeRomEntry, NativeRomSet,
    NativeTraceConfig, StereoSample, YMF271_SAMPLE_RATE_HZ, native_aspect_corrected_gui_frame,
    native_platform_json, native_playable_match_entry_script,
    native_update_aspect_corrected_gui_frame, native_window_frame_from_display,
    native_window_prefers_stronger_stacked_field, png_from_rgb888_pixels, preferred_platform_info,
    prepare_native_playable_emulator,
};
pub use protocol::{
    action_space_json, api_index_json, character_action_space_json, observation_space_json,
};
pub use zinc::{ZincConfig, ZincRuntime};
