#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePlatformInfo {
    pub execution_path: &'static str,
    pub target: &'static str,
    pub apple_silicon_preferred: bool,
    pub generic_equivalent: bool,
}

impl NativePlatformInfo {
    pub fn json(&self) -> String {
        format!(
            "{{\"execution_path\":\"{}\",\"target\":\"{}\",\"apple_silicon_preferred\":{},\"generic_equivalent\":{}}}",
            self.execution_path, self.target, self.apple_silicon_preferred, self.generic_equivalent
        )
    }
}

pub(crate) trait NativePlatformOps {
    const INFO: NativePlatformInfo;

    fn read_le_u16(bytes: &[u8]) -> u16;
    fn read_le_u32(bytes: &[u8]) -> u32;
    fn write_le_u16(value: u16) -> [u8; 2];
    fn write_le_u32(value: u32) -> [u8; 4];
}

pub fn preferred_platform_info() -> NativePlatformInfo {
    PreferredNativePlatform::INFO
}

pub fn native_platform_json() -> String {
    preferred_platform_info().json()
}

pub const APPLE_SILICON_NATIVE_TARGET: bool =
    cfg!(all(target_arch = "aarch64", target_os = "macos"));

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub(crate) type PreferredNativePlatform = apple_silicon::AppleSiliconNativePlatform;

#[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
pub(crate) type PreferredNativePlatform = generic::GenericNativePlatform;

pub use generic::GenericNativePlatform;

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub use apple_silicon::AppleSiliconNativePlatform;

pub mod generic {
    use super::{NativePlatformInfo, NativePlatformOps};

    #[derive(Clone, Copy, Debug)]
    pub struct GenericNativePlatform;

    impl NativePlatformOps for GenericNativePlatform {
        const INFO: NativePlatformInfo = NativePlatformInfo {
            execution_path: "generic",
            target: "portable",
            apple_silicon_preferred: false,
            generic_equivalent: true,
        };

        #[inline]
        fn read_le_u16(bytes: &[u8]) -> u16 {
            u16::from_le_bytes(bytes[..2].try_into().expect("u16 read length checked"))
        }

        #[inline]
        fn read_le_u32(bytes: &[u8]) -> u32 {
            u32::from_le_bytes(bytes[..4].try_into().expect("u32 read length checked"))
        }

        #[inline]
        fn write_le_u16(value: u16) -> [u8; 2] {
            value.to_le_bytes()
        }

        #[inline]
        fn write_le_u32(value: u32) -> [u8; 4] {
            value.to_le_bytes()
        }
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "macos"))]
pub mod apple_silicon {
    use super::generic::GenericNativePlatform;
    use super::{NativePlatformInfo, NativePlatformOps};

    #[derive(Clone, Copy, Debug)]
    pub struct AppleSiliconNativePlatform;

    impl NativePlatformOps for AppleSiliconNativePlatform {
        const INFO: NativePlatformInfo = NativePlatformInfo {
            execution_path: "apple_silicon",
            target: "aarch64-apple-darwin",
            apple_silicon_preferred: true,
            generic_equivalent: true,
        };

        #[inline]
        fn read_le_u16(bytes: &[u8]) -> u16 {
            GenericNativePlatform::read_le_u16(bytes)
        }

        #[inline]
        fn read_le_u32(bytes: &[u8]) -> u32 {
            GenericNativePlatform::read_le_u32(bytes)
        }

        #[inline]
        fn write_le_u16(value: u16) -> [u8; 2] {
            GenericNativePlatform::write_le_u16(value)
        }

        #[inline]
        fn write_le_u32(value: u32) -> [u8; 4] {
            GenericNativePlatform::write_le_u32(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APPLE_SILICON_NATIVE_TARGET, GenericNativePlatform, NativePlatformOps,
        PreferredNativePlatform, preferred_platform_info,
    };

    #[test]
    fn preferred_platform_matches_compile_target() {
        let info = preferred_platform_info();
        if APPLE_SILICON_NATIVE_TARGET {
            assert_eq!(info.execution_path, "apple_silicon");
            assert_eq!(info.target, "aarch64-apple-darwin");
            assert!(info.apple_silicon_preferred);
        } else {
            assert_eq!(info.execution_path, "generic");
            assert_eq!(info.target, "portable");
            assert!(!info.apple_silicon_preferred);
        }
        assert!(info.generic_equivalent);
    }

    #[test]
    fn preferred_path_preserves_generic_little_endian_access() {
        let bytes = [0xef, 0xbe, 0xad, 0xde];

        assert_eq!(
            PreferredNativePlatform::read_le_u16(&bytes),
            GenericNativePlatform::read_le_u16(&bytes)
        );
        assert_eq!(
            PreferredNativePlatform::read_le_u32(&bytes),
            GenericNativePlatform::read_le_u32(&bytes)
        );
        assert_eq!(
            PreferredNativePlatform::write_le_u16(0xbeef),
            GenericNativePlatform::write_le_u16(0xbeef)
        );
        assert_eq!(
            PreferredNativePlatform::write_le_u32(0xdead_beef),
            GenericNativePlatform::write_le_u32(0xdead_beef)
        );
    }

    #[cfg(not(all(target_arch = "aarch64", target_os = "macos")))]
    #[test]
    fn portable_targets_use_generic_platform_without_apple_symbols() {
        let info = preferred_platform_info();
        assert_eq!(info, GenericNativePlatform::INFO);

        let preferred_type = std::any::type_name::<PreferredNativePlatform>();
        assert!(preferred_type.contains("GenericNativePlatform"));
        assert!(!preferred_type.contains("AppleSiliconNativePlatform"));
    }
}
