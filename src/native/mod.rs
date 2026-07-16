pub mod backend;
pub mod bus;
pub mod cpu;
pub mod emulator;
pub mod framebuffer;
pub mod io;
pub mod platform;
pub mod romset;

pub use backend::NativeBackend;
pub use bus::NativeInputActivity;
pub use emulator::{
    NativeDisplayFrame, NativeEmulator, NativeTraceConfig, native_window_frame_from_display,
    native_window_prefers_stronger_stacked_field,
};
pub use framebuffer::png_from_rgb888_pixels;
pub use io::{NativeGpuDisplayCandidate, NativeGpuDrawCapturePredicate};
pub use platform::{
    GenericNativePlatform, NativePlatformInfo, native_platform_json, preferred_platform_info,
};
pub use romset::{
    NativeRomAssetExpectation, NativeRomAssetMatch, NativeRomAssetMismatch, NativeRomCacheReport,
    NativeRomCachedScan, NativeRomCompatibilityReport, NativeRomDuplicateAsset, NativeRomEntry,
    NativeRomManifest, NativeRomManifestEntry, NativeRomSet, bloody_roar_2_manifest,
};
