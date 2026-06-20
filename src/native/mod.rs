pub mod backend;
pub mod bus;
pub mod cpu;
pub mod emulator;
pub mod framebuffer;
pub mod io;
pub mod platform;
pub mod romset;

pub use backend::NativeBackend;
pub use emulator::{NativeDisplayFrame, NativeEmulator, NativeTraceConfig};
pub use io::NativeGpuDisplayCandidate;
pub use platform::{
    GenericNativePlatform, NativePlatformInfo, native_platform_json, preferred_platform_info,
};
pub use romset::{
    NativeRomAssetExpectation, NativeRomAssetMatch, NativeRomAssetMismatch,
    NativeRomCompatibilityReport, NativeRomDuplicateAsset, NativeRomEntry, NativeRomManifest,
    NativeRomManifestEntry, NativeRomSet, bloody_roar_2_manifest,
};
