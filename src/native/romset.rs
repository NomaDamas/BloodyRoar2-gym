use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_char, c_int, c_uint, c_ulong, c_void};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::time::UNIX_EPOCH;

use crate::backend::BackendError;
use crate::native::bus::NativeBoardAssets;

const LOCAL_FILE_HEADER_SIGNATURE: u32 = 0x0403_4b50;
const EOCD_SIGNATURE: u32 = 0x0605_4b50;
const CENTRAL_DIRECTORY_FILE_HEADER_SIGNATURE: u32 = 0x0201_4b50;
const ZIP64_EXTENDED_INFORMATION_EXTRA_FIELD: u16 = 0x0001;
const ZIP_STORED_METHOD: u16 = 0;
const ZIP_DEFLATED_METHOD: u16 = 8;
const NATIVE_ROM_CACHE_VERSION: &str = "native-rom-cache-v3";
const BLOODY_ROAR_2_GAME_ID: &str = "bldyror2";
const CAT702_ET01_CRC32: u32 = 0xa7dd_922e;
const CAT702_ET03_CRC32: u32 = 0x779b_0bfd;
const AT28C16_WORLD_CRC32: u32 = 0x01b4_2397;
const AT28C16_USA_CRC32: u32 = 0xb78d_6fc3;
const AT28C16_JAPAN_CRC32: u32 = 0x6cb5_5630;
const AT28C16_ASIA_CRC32: u32 = 0xda8c_1a64;
const ZINC_JP_FLASH1_CRC32: u32 = 0x4866_dce3;
const BLOODY_ROAR_2_MANIFEST: NativeRomManifest = NativeRomManifest {
    game_id: BLOODY_ROAR_2_GAME_ID,
    title: "Bloody Roar 2 (World)",
    hardware: "Sony ZN / PlayStation-family arcade board",
    bios_set: "coh1002e",
    source: "MAME 0.288 -listxml bldyror2/coh1002e",
    bios_assets: &SONY_ZN_BIOS_MANIFEST_ASSETS,
    game_assets: &BLOODY_ROAR_2_GAME_MANIFEST_ASSETS,
};
const ROM_NAME_ALIASES: [(&str, &str); 1] = [("coh-1002e.353", "m27c402cz-54.ic353")];
const SONY_ZN_BIOS_MANIFEST_ASSETS: [NativeRomManifestEntry; 3] = [
    NativeRomManifestEntry {
        name: "m27c402cz-54.ic353",
        role: "zn_boot_rom",
        source_set: "coh1002e",
        required: true,
        expected_size: 524_288,
        expected_crc32: Some(0x910f_3a8b),
        expected_sha1: Some("cd68532967a25f476a6d73473ec6b6f4df2e1689"),
        region: "maincpu:rom",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "et01.ic652",
        role: "cat702_security_eeprom",
        source_set: "coh1002e",
        required: true,
        expected_size: 8,
        expected_crc32: Some(0xa7dd_922e),
        expected_sha1: Some("1069c1d9015028a51a1b314cfacb014ea90aa425"),
        region: "cat702_1",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "78081g503.ic655",
        role: "protection_microcontroller",
        source_set: "coh1002e",
        required: true,
        expected_size: 8_192,
        expected_crc32: None,
        expected_sha1: None,
        region: "upd78081",
        offset: "0",
        dump_status: "nodump",
        merge: None,
    },
];
const BLOODY_ROAR_2_GAME_MANIFEST_ASSETS: [NativeRomManifestEntry; 11] = [
    NativeRomManifestEntry {
        name: "flash0.021",
        role: "program_flash",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 2_097_152,
        expected_crc32: Some(0xfa76_02e1),
        expected_sha1: Some("6fb6af09656fbb86d2abda35804b2ed4a4cd7461"),
        region: "bankedroms",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "flash1.024",
        role: "program_flash",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 2_097_152,
        expected_crc32: Some(0x0346_5a69),
        expected_sha1: Some("7c29aff2bf19c379873d3927c260892c78281882"),
        region: "bankedroms",
        offset: "200000",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "rom-1a.028",
        role: "banked_mask_rom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 4_194_304,
        expected_crc32: Some(0x0e71_1461),
        expected_sha1: Some("1d0bd80e6885432ef0623babde28e5760b714bfa"),
        region: "bankedroms",
        offset: "800000",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "rom-1b.29",
        role: "banked_mask_rom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 4_194_304,
        expected_crc32: Some(0x0cf1_53f9),
        expected_sha1: Some("53bb9f8642079f56d8e925792b069362df666819"),
        region: "bankedroms",
        offset: "c00000",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "rom-2a.026",
        role: "banked_mask_rom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 4_194_304,
        expected_crc32: Some(0xb71d_955d),
        expected_sha1: Some("49fce452c70ceafc8a149fa9ff073589b7261882"),
        region: "bankedroms",
        offset: "1000000",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "rom-2b.210",
        role: "banked_mask_rom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 4_194_304,
        expected_crc32: Some(0x8995_9dde),
        expected_sha1: Some("99d54b9876f38f5e625334bbd1439618cdf01d56"),
        region: "bankedroms",
        offset: "1400000",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "br2_u0412.412",
        role: "audio_cpu_even",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 524_288,
        expected_crc32: Some(0xe254_dd8a),
        expected_sha1: Some("5b8fcafcf2176e0b55efcf37799d7c0d97e01bdc"),
        region: "audiocpu",
        offset: "1",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "br2_u049.049",
        role: "audio_cpu_odd",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 524_288,
        expected_crc32: Some(0x10dc_855b),
        expected_sha1: Some("4e6e3a71911c8976ae07c2b6cac5a36f98193def"),
        region: "audiocpu",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "rom-3.336",
        role: "ymf_sample_rom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 4_194_304,
        expected_crc32: Some(0xb74c_c4d1),
        expected_sha1: Some("eb5485582a12959ae06927a2f1d8a7e63e0f956f"),
        region: "ymf",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "at28c16_world",
        role: "settings_eeprom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 2_048,
        expected_crc32: Some(0x01b4_2397),
        expected_sha1: Some("853553a38e81e64a17c040173b29c7bfd6f79f31"),
        region: "at28c16",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
    NativeRomManifestEntry {
        name: "et03",
        role: "cat702_security_eeprom",
        source_set: BLOODY_ROAR_2_GAME_ID,
        required: true,
        expected_size: 8,
        expected_crc32: Some(0x779b_0bfd),
        expected_sha1: Some("76a188c78083bbb2740379d53143e1efaf637b85"),
        region: "cat702_2",
        offset: "0",
        dump_status: "good",
        merge: None,
    },
];
const BLOODY_ROAR_2_REQUIRED_ASSETS: [NativeRomAssetExpectation; 14] = [
    NativeRomAssetExpectation {
        name: "m27c402cz-54.ic353",
        role: "zn2_boot_rom",
        expected_size: 524_288,
        expected_crc32: Some(0x910f_3a8b),
    },
    NativeRomAssetExpectation {
        name: "et01.ic652",
        role: "security_eeprom",
        expected_size: 8,
        expected_crc32: Some(0xa7dd_922e),
    },
    NativeRomAssetExpectation {
        name: "78081g503.ic655",
        role: "protection_microcontroller",
        expected_size: 8_192,
        expected_crc32: None,
    },
    NativeRomAssetExpectation {
        name: "flash0.021",
        role: "program_flash",
        expected_size: 2_097_152,
        expected_crc32: Some(0xfa76_02e1),
    },
    NativeRomAssetExpectation {
        name: "flash1.024",
        role: "program_flash",
        expected_size: 2_097_152,
        expected_crc32: Some(0x0346_5a69),
    },
    NativeRomAssetExpectation {
        name: "rom-1a.028",
        role: "mask_rom",
        expected_size: 4_194_304,
        expected_crc32: Some(0x0e71_1461),
    },
    NativeRomAssetExpectation {
        name: "rom-1b.29",
        role: "mask_rom",
        expected_size: 4_194_304,
        expected_crc32: Some(0x0cf1_53f9),
    },
    NativeRomAssetExpectation {
        name: "rom-2a.026",
        role: "mask_rom",
        expected_size: 4_194_304,
        expected_crc32: Some(0xb71d_955d),
    },
    NativeRomAssetExpectation {
        name: "rom-2b.210",
        role: "mask_rom",
        expected_size: 4_194_304,
        expected_crc32: Some(0x8995_9dde),
    },
    NativeRomAssetExpectation {
        name: "br2_u0412.412",
        role: "sample_rom",
        expected_size: 524_288,
        expected_crc32: Some(0xe254_dd8a),
    },
    NativeRomAssetExpectation {
        name: "br2_u049.049",
        role: "sample_rom",
        expected_size: 524_288,
        expected_crc32: Some(0x10dc_855b),
    },
    NativeRomAssetExpectation {
        name: "rom-3.336",
        role: "mask_rom",
        expected_size: 4_194_304,
        expected_crc32: Some(0xb74c_c4d1),
    },
    NativeRomAssetExpectation {
        name: "at28c16_world",
        role: "settings_eeprom",
        expected_size: 2_048,
        expected_crc32: Some(0x01b4_2397),
    },
    NativeRomAssetExpectation {
        name: "et03",
        role: "security_eeprom",
        expected_size: 8,
        expected_crc32: Some(0x779b_0bfd),
    },
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRomManifest {
    pub game_id: &'static str,
    pub title: &'static str,
    pub hardware: &'static str,
    pub bios_set: &'static str,
    pub source: &'static str,
    pub bios_assets: &'static [NativeRomManifestEntry],
    pub game_assets: &'static [NativeRomManifestEntry],
}

impl NativeRomManifest {
    pub fn all_assets(&self) -> impl Iterator<Item = &NativeRomManifestEntry> {
        self.bios_assets.iter().chain(self.game_assets.iter())
    }

    fn json(&self) -> String {
        let bios_assets = self
            .bios_assets
            .iter()
            .map(NativeRomManifestEntry::json)
            .collect::<Vec<_>>()
            .join(",");
        let game_assets = self
            .game_assets
            .iter()
            .map(NativeRomManifestEntry::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"game_id\":\"{}\",\"title\":\"{}\",\"hardware\":\"{}\",\"bios_set\":\"{}\",\"source\":\"{}\",\"bios_assets\":[{}],\"game_assets\":[{}]}}",
            self.game_id,
            escape_json(self.title),
            escape_json(self.hardware),
            self.bios_set,
            escape_json(self.source),
            bios_assets,
            game_assets
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRomManifestEntry {
    pub name: &'static str,
    pub role: &'static str,
    pub source_set: &'static str,
    pub required: bool,
    pub expected_size: u64,
    pub expected_crc32: Option<u32>,
    pub expected_sha1: Option<&'static str>,
    pub region: &'static str,
    pub offset: &'static str,
    pub dump_status: &'static str,
    pub merge: Option<&'static str>,
}

impl NativeRomManifestEntry {
    fn json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"role\":\"{}\",\"source_set\":\"{}\",\"required\":{},\"optional\":{},\"expected_size\":{},\"expected_crc32\":{},\"expected_sha1\":{},\"region\":\"{}\",\"offset\":\"{}\",\"dump_status\":\"{}\",\"merge\":{}}}",
            self.name,
            self.role,
            self.source_set,
            self.required,
            !self.required,
            self.expected_size,
            optional_crc_json(self.expected_crc32),
            optional_str_json(self.expected_sha1),
            self.region,
            self.offset,
            self.dump_status,
            optional_str_json(self.merge)
        )
    }
}

pub fn bloody_roar_2_manifest() -> &'static NativeRomManifest {
    &BLOODY_ROAR_2_MANIFEST
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeRomAssetExpectation {
    pub name: &'static str,
    pub role: &'static str,
    pub expected_size: u64,
    pub expected_crc32: Option<u32>,
}

impl NativeRomAssetExpectation {
    fn json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"role\":\"{}\",\"expected_size\":{},\"expected_crc32\":{}}}",
            self.name,
            self.role,
            self.expected_size,
            optional_crc_json(self.expected_crc32)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRomAssetMismatch {
    pub name: String,
    pub role: &'static str,
    pub expected_size: u64,
    pub actual_size: u64,
    pub expected_crc32: Option<u32>,
    pub actual_crc32: u32,
}

impl NativeRomAssetMismatch {
    fn json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"role\":\"{}\",\"expected_size\":{},\"actual_size\":{},\"expected_crc32\":{},\"actual_crc32\":\"{:08x}\"}}",
            escape_json(&self.name),
            self.role,
            self.expected_size,
            self.actual_size,
            optional_crc_json(self.expected_crc32),
            self.actual_crc32
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRomAssetMatch {
    pub provided_name: String,
    pub manifest_name: Option<&'static str>,
    pub asset_group: &'static str,
    pub source_set: Option<&'static str>,
    pub role: Option<&'static str>,
    pub expected_size: Option<u64>,
    pub actual_size: u64,
    pub expected_crc32: Option<u32>,
    pub actual_crc32: u32,
    pub status: &'static str,
    pub issues: Vec<String>,
}

impl NativeRomAssetMatch {
    fn json(&self) -> String {
        format!(
            "{{\"provided_name\":\"{}\",\"manifest_name\":{},\"asset_group\":\"{}\",\"source_set\":{},\"role\":{},\"expected_size\":{},\"actual_size\":{},\"expected_crc32\":{},\"actual_crc32\":\"{:08x}\",\"status\":\"{}\",\"issues\":[{}]}}",
            escape_json(&self.provided_name),
            optional_str_json(self.manifest_name),
            self.asset_group,
            optional_str_json(self.source_set),
            optional_str_json(self.role),
            optional_u64_json(self.expected_size),
            self.actual_size,
            optional_crc_json(self.expected_crc32),
            self.actual_crc32,
            self.status,
            json_string_array(&self.issues)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRomDuplicateAsset {
    pub name: String,
    pub normalized_name: String,
    pub occurrences: usize,
    pub entries: Vec<NativeRomEntry>,
}

impl NativeRomDuplicateAsset {
    fn json(&self) -> String {
        let entries = self
            .entries
            .iter()
            .map(NativeRomEntry::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"name\":\"{}\",\"normalized_name\":\"{}\",\"occurrences\":{},\"entries\":[{}]}}",
            escape_json(&self.name),
            escape_json(&self.normalized_name),
            self.occurrences,
            entries
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRomCompatibilityReport {
    pub game_id: &'static str,
    pub manifest_source: &'static str,
    pub present_assets: Vec<String>,
    pub present_bios_assets: Vec<String>,
    pub present_game_assets: Vec<String>,
    pub missing_required_assets: Vec<String>,
    pub unknown_assets: Vec<String>,
    pub mismatched_assets: Vec<NativeRomAssetMismatch>,
    pub asset_matches: Vec<NativeRomAssetMatch>,
    pub duplicate_assets: Vec<NativeRomDuplicateAsset>,
    pub expectations: Vec<NativeRomAssetExpectation>,
}

impl NativeRomCompatibilityReport {
    pub fn missing_all_required_assets() -> Self {
        Self {
            game_id: BLOODY_ROAR_2_GAME_ID,
            manifest_source: BLOODY_ROAR_2_MANIFEST.source,
            present_assets: Vec::new(),
            present_bios_assets: Vec::new(),
            present_game_assets: Vec::new(),
            missing_required_assets: BLOODY_ROAR_2_REQUIRED_ASSETS
                .iter()
                .map(|asset| asset.name.to_string())
                .collect(),
            unknown_assets: Vec::new(),
            mismatched_assets: Vec::new(),
            asset_matches: Vec::new(),
            duplicate_assets: Vec::new(),
            expectations: BLOODY_ROAR_2_REQUIRED_ASSETS.to_vec(),
        }
    }

    pub fn compatible(&self) -> bool {
        self.missing_required_assets.is_empty()
            && self.unknown_assets.is_empty()
            && self.mismatched_assets.is_empty()
            && !self.has_duplicate_required_assets()
    }

    pub fn native_runtime_usable(&self) -> bool {
        !self.has_duplicate_required_assets()
            && !self
                .missing_required_assets
                .iter()
                .any(|asset| native_runtime_required_asset_missing(asset))
            && !self
                .mismatched_assets
                .iter()
                .any(|mismatch| !native_runtime_allowed_mismatch(mismatch))
    }

    pub fn has_duplicate_required_assets(&self) -> bool {
        self.duplicate_assets
            .iter()
            .any(|duplicate| is_required_asset_name(&duplicate.normalized_name))
    }

    pub fn summary_json(&self) -> String {
        let mismatched_assets = self
            .mismatched_assets
            .iter()
            .map(NativeRomAssetMismatch::json)
            .collect::<Vec<_>>()
            .join(",");
        let known_variants = self.known_variants_json();
        format!(
            "{{\"game_id\":\"{}\",\"manifest_source\":\"{}\",\"compatible\":{},\"native_runtime_usable\":{},\"known_variants\":[{}],\"missing_required_assets\":[{}],\"mismatched_assets\":[{}],\"unknown_asset_count\":{},\"duplicate_required_assets\":{}}}",
            self.game_id,
            escape_json(self.manifest_source),
            self.compatible(),
            self.native_runtime_usable(),
            known_variants,
            json_string_array(&self.missing_required_assets),
            mismatched_assets,
            self.unknown_assets.len(),
            self.has_duplicate_required_assets()
        )
    }

    fn json(&self) -> String {
        let present_assets = json_string_array(&self.present_assets);
        let present_bios_assets = json_string_array(&self.present_bios_assets);
        let present_game_assets = json_string_array(&self.present_game_assets);
        let missing_required_assets = json_string_array(&self.missing_required_assets);
        let unknown_assets = json_string_array(&self.unknown_assets);
        let mismatched_assets = self
            .mismatched_assets
            .iter()
            .map(NativeRomAssetMismatch::json)
            .collect::<Vec<_>>()
            .join(",");
        let asset_matches = self
            .asset_matches
            .iter()
            .map(NativeRomAssetMatch::json)
            .collect::<Vec<_>>()
            .join(",");
        let duplicate_assets = self
            .duplicate_assets
            .iter()
            .map(NativeRomDuplicateAsset::json)
            .collect::<Vec<_>>()
            .join(",");
        let expectations = self
            .expectations
            .iter()
            .map(NativeRomAssetExpectation::json)
            .collect::<Vec<_>>()
            .join(",");
        let known_variants = self.known_variants_json();
        format!(
            "{{\"game_id\":\"{}\",\"manifest_source\":\"{}\",\"compatible\":{},\"native_runtime_usable\":{},\"known_variants\":[{}],\"present_assets\":[{}],\"present_bios_assets\":[{}],\"present_game_assets\":[{}],\"missing_required_assets\":[{}],\"unknown_assets\":[{}],\"mismatched_assets\":[{}],\"asset_matches\":[{}],\"has_duplicate_required_assets\":{},\"duplicate_assets\":[{}],\"expectations\":[{}]}}",
            self.game_id,
            escape_json(self.manifest_source),
            self.compatible(),
            self.native_runtime_usable(),
            known_variants,
            present_assets,
            present_bios_assets,
            present_game_assets,
            missing_required_assets,
            unknown_assets,
            mismatched_assets,
            asset_matches,
            self.has_duplicate_required_assets(),
            duplicate_assets,
            expectations
        )
    }

    fn known_variants_json(&self) -> String {
        let variants = self
            .known_variants()
            .into_iter()
            .map(|variant| format!("\"{}\"", escape_json(variant)))
            .collect::<Vec<_>>();
        variants.join(",")
    }

    fn known_variants(&self) -> Vec<&'static str> {
        let has_zinc_jp_flash = self.mismatched_assets.iter().any(|mismatch| {
            mismatch.name.eq_ignore_ascii_case("flash1.024")
                && mismatch.actual_crc32 == ZINC_JP_FLASH1_CRC32
        });

        if has_zinc_jp_flash {
            vec!["zinc_jp_bundle_flash_variant"]
        } else {
            Vec::new()
        }
    }
}

fn native_runtime_required_asset_missing(asset: &str) -> bool {
    !matches!(
        normalized_file_name(asset).as_str(),
        "et01.ic652" | "78081g503.ic655" | "at28c16_world" | "et03"
    )
}

fn native_runtime_allowed_mismatch(mismatch: &NativeRomAssetMismatch) -> bool {
    mismatch.name.eq_ignore_ascii_case("flash1.024")
        && mismatch.actual_size == mismatch.expected_size
        && mismatch.actual_crc32 == ZINC_JP_FLASH1_CRC32
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeRomEntry {
    pub name: String,
    pub uncompressed_size: u64,
    pub compressed_size: u64,
    pub crc32: u32,
    pub compression_method: u16,
    pub local_header_offset: Option<u64>,
}

impl NativeRomEntry {
    pub fn compression_method_name(&self) -> &'static str {
        match self.compression_method {
            ZIP_STORED_METHOD => "stored",
            1 => "shrunk",
            6 => "imploded",
            ZIP_DEFLATED_METHOD => "deflated",
            9 => "deflate64",
            12 => "bzip2",
            14 => "lzma",
            93 => "zstd",
            98 => "ppmd",
            _ => "unknown",
        }
    }

    fn json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"uncompressed_size\":{},\"compressed_size\":{},\"crc32\":\"{:08x}\",\"compression_method\":{},\"compression_method_name\":\"{}\",\"local_header_offset\":{}}}",
            escape_json(&self.name),
            self.uncompressed_size,
            self.compressed_size,
            self.crc32,
            self.compression_method,
            self.compression_method_name(),
            optional_u64_json(self.local_header_offset)
        )
    }
}

#[derive(Clone, Debug)]
pub struct NativeRomSet {
    pub path: PathBuf,
    pub entries: Vec<String>,
    pub entry_metadata: Vec<NativeRomEntry>,
    archive_bytes: Option<Vec<u8>>,
    entry_bytes_cache: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl NativeRomSet {
    pub fn scan(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let path = path.into();
        let metadata = fs::metadata(&path).map_err(|error| {
            BackendError::new(format!("failed to inspect {}: {error}", path.display()))
        })?;

        if metadata.is_dir() {
            return Self::scan_dir(path);
        }

        Self::inspect(path)
    }

    pub fn scan_cached(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let path = path.into();
        let metadata = fs::metadata(&path).map_err(|error| {
            BackendError::new(format!("failed to inspect {}: {error}", path.display()))
        })?;

        if metadata.is_dir() {
            return Self::scan_dir(path);
        }

        let cache = NativeRomCache::for_source(&path, &metadata);
        if let Some(romset) = Self::try_scan_cached_romset(&cache) {
            return Ok(romset);
        }

        let source = Self::inspect(&path)?;
        materialize_cached_romset(&source, &cache)?;
        Self::scan_dir(cache.rom_dir)
    }

    pub fn inspect(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let path = path.into();
        let bytes = fs::read(&path).map_err(|error| {
            BackendError::new(format!("failed to read {}: {error}", path.display()))
        })?;
        let entry_metadata = parse_zip_entries_with_nested_archives(&path, &bytes)?;
        let entries = entry_metadata
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        Ok(Self {
            path,
            entries,
            entry_metadata,
            archive_bytes: Some(bytes),
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        })
    }

    fn scan_dir(path: PathBuf) -> Result<Self, BackendError> {
        let files = collect_files_sorted(&path)?;
        let mut entry_metadata = Vec::new();

        for file in files {
            if is_zip_path(&file) {
                let bytes = fs::read(&file).map_err(|error| {
                    BackendError::new(format!("failed to read {}: {error}", file.display()))
                })?;
                let archive_name = file
                    .strip_prefix(&path)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");
                for mut entry in parse_zip_entries_with_nested_archives(&file, &bytes)? {
                    if entry.name.ends_with('/') {
                        continue;
                    }
                    entry.name = format!("{archive_name}/{}", entry.name);
                    entry_metadata.push(entry);
                }
                continue;
            }

            let relative = file.strip_prefix(&path).unwrap_or(&file);
            let bytes = fs::read(&file).map_err(|error| {
                BackendError::new(format!("failed to read {}: {error}", file.display()))
            })?;
            entry_metadata.push(NativeRomEntry {
                name: relative.to_string_lossy().replace('\\', "/"),
                uncompressed_size: bytes.len() as u64,
                compressed_size: bytes.len() as u64,
                crc32: crc32(&bytes),
                compression_method: 0,
                local_header_offset: None,
            });
        }

        let entries = entry_metadata
            .iter()
            .map(|entry| entry.name.clone())
            .collect();

        Ok(Self {
            path,
            entries,
            entry_metadata,
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        })
    }

    fn try_scan_cached_romset(cache: &NativeRomCache) -> Option<Self> {
        if !cache.ready_file.is_file() || !cache.rom_dir.is_dir() {
            return None;
        }

        let romset = Self::scan_dir(cache.rom_dir.clone()).ok()?;
        (!romset.entry_metadata.is_empty()).then_some(romset)
    }

    pub fn load_boot_rom(&self) -> Result<Vec<u8>, BackendError> {
        self.load_manifest_asset("m27c402cz-54.ic353")
            .map_err(|_| BackendError::new("no supported boot ROM entry found in ROM set"))
    }

    pub fn load_banked_roms(&self) -> Result<Vec<u8>, BackendError> {
        self.load_manifest_region("bankedroms")
    }

    pub fn load_board_assets(&self) -> NativeBoardAssets {
        let mut assets = NativeBoardAssets {
            cat702_1: self.load_exact_8_asset("et01.ic652"),
            cat702_2: self.load_exact_8_asset("et03"),
            at28c16: self.load_exact_asset("at28c16_world", 2048),
        };

        if assets.cat702_1.is_some() && assets.cat702_2.is_some() && assets.at28c16.is_some() {
            return assets;
        }

        for path in board_asset_candidates(&self.path) {
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };

            if assets.cat702_1.is_none() {
                assets.cat702_1 = find_crc32_window_8(&bytes, CAT702_ET01_CRC32);
            }
            if assets.cat702_2.is_none() {
                assets.cat702_2 = find_crc32_window_8(&bytes, CAT702_ET03_CRC32);
            }
            if assets.at28c16.is_none() {
                assets.at28c16 = at28c16_fallback_bytes(&path, &bytes);
            }

            if assets.cat702_1.is_some() && assets.cat702_2.is_some() && assets.at28c16.is_some() {
                break;
            }
        }

        assets
    }

    pub fn load_manifest_region(&self, region: &str) -> Result<Vec<u8>, BackendError> {
        let region_assets = BLOODY_ROAR_2_MANIFEST
            .game_assets
            .iter()
            .filter(|entry| entry.region == region)
            .collect::<Vec<_>>();

        if region_assets.is_empty() {
            return Err(BackendError::new(format!(
                "no manifest assets are defined for region {region}"
            )));
        }

        let mut image = Vec::new();
        let mut loaded_assets = 0usize;
        for manifest_entry in region_assets {
            let Some(entry) = self.find_entry(manifest_entry.name) else {
                continue;
            };
            let offset = parse_manifest_offset(manifest_entry.offset)?;
            let bytes = self.load_entry_bytes(entry)?;
            let end = offset.checked_add(bytes.len()).ok_or_else(|| {
                BackendError::new(format!(
                    "region {region} asset {} overflows address space",
                    manifest_entry.name
                ))
            })?;
            if image.len() < end {
                image.resize(end, 0);
            }
            image[offset..end].copy_from_slice(&bytes);
            loaded_assets += 1;
        }

        if loaded_assets == 0 {
            return Err(BackendError::new(format!(
                "no local assets found for manifest region {region}"
            )));
        }

        Ok(image)
    }

    pub fn load_manifest_asset(&self, manifest_name: &str) -> Result<Vec<u8>, BackendError> {
        let entry = self.find_entry(manifest_name).ok_or_else(|| {
            BackendError::new(format!("manifest asset {manifest_name} is missing"))
        })?;
        self.load_entry_bytes(entry)
    }

    pub fn bloody_roar_2_compatibility(&self) -> NativeRomCompatibilityReport {
        let mut present_assets = Vec::new();
        let mut present_bios_assets = Vec::new();
        let mut present_game_assets = Vec::new();
        let mut missing_required_assets = Vec::new();
        let mut unknown_assets = Vec::new();
        let mut mismatched_assets = Vec::new();
        let mut asset_matches = Vec::new();

        for manifest_entry in BLOODY_ROAR_2_MANIFEST.all_assets() {
            match self.find_entry(manifest_entry.name) {
                Some(entry) => {
                    present_assets.push(manifest_entry.name.to_string());
                    if manifest_asset_group(manifest_entry) == "bios" {
                        present_bios_assets.push(manifest_entry.name.to_string());
                    } else {
                        present_game_assets.push(manifest_entry.name.to_string());
                    }

                    if entry_mismatches_manifest(entry, manifest_entry) {
                        mismatched_assets.push(NativeRomAssetMismatch {
                            name: manifest_entry.name.to_string(),
                            role: manifest_entry.role,
                            expected_size: manifest_entry.expected_size,
                            actual_size: entry.uncompressed_size,
                            expected_crc32: manifest_entry.expected_crc32,
                            actual_crc32: entry.crc32,
                        });
                    }
                }
                None if manifest_entry.required => {
                    missing_required_assets.push(manifest_entry.name.to_string());
                }
                None => {}
            }
        }

        for entry in self
            .entry_metadata
            .iter()
            .filter(|entry| !entry.name.ends_with('/'))
        {
            let asset_match = manifest_match_for_entry(entry);
            if asset_match.status == "unknown" {
                unknown_assets.push(entry.name.clone());
            }
            asset_matches.push(asset_match);
        }

        NativeRomCompatibilityReport {
            game_id: BLOODY_ROAR_2_GAME_ID,
            manifest_source: BLOODY_ROAR_2_MANIFEST.source,
            present_assets,
            present_bios_assets,
            present_game_assets,
            missing_required_assets,
            unknown_assets,
            mismatched_assets,
            asset_matches,
            duplicate_assets: self.duplicate_assets(),
            expectations: BLOODY_ROAR_2_REQUIRED_ASSETS.to_vec(),
        }
    }

    pub fn compatibility_report(&self) -> NativeRomCompatibilityReport {
        self.bloody_roar_2_compatibility()
    }

    pub fn bloody_roar_2_manifest(&self) -> &'static NativeRomManifest {
        &BLOODY_ROAR_2_MANIFEST
    }

    pub fn json(&self) -> String {
        let entries = json_string_array(&self.entries);
        let entry_metadata = self
            .entry_metadata
            .iter()
            .map(NativeRomEntry::json)
            .collect::<Vec<_>>()
            .join(",");
        let duplicate_assets = self.duplicate_assets();
        let duplicate_assets_json = duplicate_assets
            .iter()
            .map(NativeRomDuplicateAsset::json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"path\":\"{}\",\"entry_count\":{},\"entries\":[{}],\"entry_metadata\":[{}],\"has_duplicate_assets\":{},\"duplicate_asset_count\":{},\"duplicate_assets\":[{}],\"bloody_roar_2_manifest\":{},\"bloody_roar_2_compatibility\":{}}}",
            escape_json(&self.path.display().to_string()),
            self.entries.len(),
            entries,
            entry_metadata,
            !duplicate_assets.is_empty(),
            duplicate_assets.len(),
            duplicate_assets_json,
            self.bloody_roar_2_manifest().json(),
            self.bloody_roar_2_compatibility().json()
        )
    }

    fn find_entry(&self, name: &str) -> Option<&NativeRomEntry> {
        self.entry_metadata
            .iter()
            .find(|entry| asset_names_match(&entry.name, name))
    }

    fn load_entry_bytes(&self, entry: &NativeRomEntry) -> Result<Vec<u8>, BackendError> {
        let cache_key = format!("entry:{}", normalized_asset_name(&entry.name));
        if let Some(bytes) = self.cached_entry_bytes(&cache_key) {
            return Ok(bytes);
        }

        let bytes = if self.path.is_dir() {
            read_scanned_entry_bytes(&self.path, &entry.name)?
        } else if let Some((archive_name, inner_entry)) = split_scanned_zip_entry(&entry.name) {
            if let Some(archive_bytes) = self.archive_bytes.as_deref() {
                self.load_nested_zip_entry_from_archive(archive_bytes, archive_name, inner_entry)?
            } else {
                read_nested_zip_entry(&self.path, archive_name, inner_entry)?
            }
        } else if let Some(archive_bytes) = self.archive_bytes.as_deref() {
            read_zip_entry_from_bytes(&self.path, archive_bytes, entry)?
        } else {
            read_zip_entry(&self.path, entry)?
        };

        self.cache_entry_bytes(cache_key, &bytes);
        Ok(bytes)
    }

    fn load_nested_zip_entry_from_archive(
        &self,
        outer_archive: &[u8],
        archive_entry: &str,
        inner_entry: &str,
    ) -> Result<Vec<u8>, BackendError> {
        let archive_cache_key = format!("archive:{}", normalized_asset_name(archive_entry));
        let nested_archive = if let Some(bytes) = self.cached_entry_bytes(&archive_cache_key) {
            bytes
        } else {
            let outer_entries = parse_zip_entries(outer_archive)?;
            let outer_entry = outer_entries
                .iter()
                .find(|entry| {
                    normalized_asset_name(&entry.name) == normalized_asset_name(archive_entry)
                })
                .ok_or_else(|| {
                    BackendError::new(format!(
                        "nested archive {archive_entry} is missing from {}",
                        self.path.display()
                    ))
                })?;
            let bytes = read_zip_entry_from_bytes(&self.path, outer_archive, outer_entry)?;
            self.cache_entry_bytes(archive_cache_key, &bytes);
            bytes
        };

        let nested_entries = parse_zip_entries(&nested_archive)?;
        let nested_entry = nested_entries
            .iter()
            .find(|entry| asset_names_match(&entry.name, inner_entry))
            .ok_or_else(|| {
                BackendError::new(format!(
                    "nested ZIP entry {inner_entry} is missing from {archive_entry}"
                ))
            })?;
        read_zip_entry_from_bytes(&self.path, &nested_archive, nested_entry)
    }

    fn cached_entry_bytes(&self, cache_key: &str) -> Option<Vec<u8>> {
        self.entry_bytes_cache.borrow().get(cache_key).cloned()
    }

    fn cache_entry_bytes(&self, cache_key: String, bytes: &[u8]) {
        self.entry_bytes_cache
            .borrow_mut()
            .insert(cache_key, bytes.to_vec());
    }

    fn load_exact_8_asset(&self, manifest_name: &str) -> Option<[u8; 8]> {
        let bytes = self.load_manifest_asset(manifest_name).ok()?;
        exact_8_bytes(&bytes)
    }

    fn load_exact_asset(&self, manifest_name: &str, expected_len: usize) -> Option<Vec<u8>> {
        let bytes = self.load_manifest_asset(manifest_name).ok()?;
        (bytes.len() == expected_len).then_some(bytes)
    }

    pub fn duplicate_assets(&self) -> Vec<NativeRomDuplicateAsset> {
        duplicate_assets(&self.entry_metadata)
    }
}

fn duplicate_assets(entries: &[NativeRomEntry]) -> Vec<NativeRomDuplicateAsset> {
    let mut by_name: BTreeMap<String, Vec<&NativeRomEntry>> = BTreeMap::new();
    for entry in entries {
        if entry.name.ends_with('/') {
            continue;
        }
        by_name
            .entry(duplicate_asset_key(&entry.name))
            .or_default()
            .push(entry);
    }

    by_name
        .into_iter()
        .filter_map(|(normalized_name, entries)| {
            if entries.len() < 2 {
                return None;
            }

            Some(NativeRomDuplicateAsset {
                name: entries[0].name.clone(),
                normalized_name,
                occurrences: entries.len(),
                entries: entries.into_iter().cloned().collect(),
            })
        })
        .collect()
}

#[derive(Clone, Debug)]
struct NativeRomCache {
    base_dir: PathBuf,
    rom_dir: PathBuf,
    ready_file: PathBuf,
    source_path: PathBuf,
    source_len: u64,
    source_modified_ns: u128,
}

impl NativeRomCache {
    fn for_source(path: &Path, metadata: &fs::Metadata) -> Self {
        let source_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let source_len = metadata.len();
        let source_modified_ns = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let source_identity = format!(
            "{}:{source_len}:{source_modified_ns}",
            source_path.display()
        );
        let identity_crc32 = crc32(source_identity.as_bytes());
        let cache_key = format!(
            "{NATIVE_ROM_CACHE_VERSION}-{identity_crc32:08x}-{source_len:016x}-{source_modified_ns:032x}"
        );
        let base_dir = PathBuf::from("target")
            .join("native-rom-cache")
            .join(cache_key);
        let rom_dir = base_dir.join("roms");
        let ready_file = base_dir.join("READY");

        Self {
            base_dir,
            rom_dir,
            ready_file,
            source_path,
            source_len,
            source_modified_ns,
        }
    }

    fn temp_dir(&self) -> PathBuf {
        let mut name = self
            .base_dir
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| NATIVE_ROM_CACHE_VERSION.to_string());
        name.push_str(&format!(".tmp-{}", process::id()));
        self.base_dir
            .parent()
            .unwrap_or_else(|| Path::new("target/native-rom-cache"))
            .join(name)
    }

    fn ready_contents(&self, written_assets: &[String]) -> String {
        format!(
            "version={NATIVE_ROM_CACHE_VERSION}\nsource={}\nsource_len={}\nsource_modified_ns={}\nasset_count={}\nassets={}\n",
            self.source_path.display(),
            self.source_len,
            self.source_modified_ns,
            written_assets.len(),
            written_assets.join(",")
        )
    }
}

fn materialize_cached_romset(
    source: &NativeRomSet,
    cache: &NativeRomCache,
) -> Result<(), BackendError> {
    if let Some(parent) = cache.base_dir.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            BackendError::new(format!(
                "failed to create native ROM cache root {}: {error}",
                parent.display()
            ))
        })?;
    }

    let temp_dir = cache.temp_dir();
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).map_err(|error| {
            BackendError::new(format!(
                "failed to clear stale native ROM cache temp dir {}: {error}",
                temp_dir.display()
            ))
        })?;
    }

    let temp_rom_dir = temp_dir.join("roms");
    fs::create_dir_all(&temp_rom_dir).map_err(|error| {
        BackendError::new(format!(
            "failed to create native ROM cache dir {}: {error}",
            temp_rom_dir.display()
        ))
    })?;

    let mut written_assets = Vec::new();
    for manifest_entry in BLOODY_ROAR_2_MANIFEST.all_assets() {
        if source.find_entry(manifest_entry.name).is_none() {
            continue;
        }

        let bytes = source.load_manifest_asset(manifest_entry.name)?;
        write_cached_rom_asset(
            &temp_rom_dir,
            &mut written_assets,
            manifest_entry.name,
            &bytes,
        )?;
    }

    let board_assets = source.load_board_assets();
    if let Some(bytes) = board_assets.cat702_1 {
        write_cached_rom_asset(&temp_rom_dir, &mut written_assets, "et01.ic652", &bytes)?;
    }
    if let Some(bytes) = board_assets.cat702_2 {
        write_cached_rom_asset(&temp_rom_dir, &mut written_assets, "et03", &bytes)?;
    }
    if let Some(bytes) = board_assets.at28c16 {
        let name = if crc32(&bytes) == AT28C16_WORLD_CRC32 {
            "at28c16_world"
        } else {
            "bldyror2.cfg"
        };
        write_cached_rom_asset(&temp_rom_dir, &mut written_assets, name, &bytes)?;
    }

    if written_assets.is_empty() {
        return Err(BackendError::new(format!(
            "no Bloody Roar 2 manifest assets were found in {}",
            source.path.display()
        )));
    }

    fs::write(
        temp_dir.join("READY"),
        cache.ready_contents(&written_assets),
    )
    .map_err(|error| {
        BackendError::new(format!(
            "failed to write native ROM cache marker under {}: {error}",
            temp_dir.display()
        ))
    })?;

    if cache.base_dir.exists() {
        fs::remove_dir_all(&cache.base_dir).map_err(|error| {
            BackendError::new(format!(
                "failed to replace native ROM cache dir {}: {error}",
                cache.base_dir.display()
            ))
        })?;
    }
    fs::rename(&temp_dir, &cache.base_dir).map_err(|error| {
        BackendError::new(format!(
            "failed to activate native ROM cache {}: {error}",
            cache.base_dir.display()
        ))
    })
}

fn write_cached_rom_asset(
    cache_rom_dir: &Path,
    written_assets: &mut Vec<String>,
    name: &str,
    bytes: &[u8],
) -> Result<(), BackendError> {
    if written_assets.iter().any(|asset| asset == name) {
        return Ok(());
    }

    let output = cache_rom_dir.join(name);
    fs::write(&output, bytes).map_err(|error| {
        BackendError::new(format!(
            "failed to write cached native ROM asset {}: {error}",
            output.display()
        ))
    })?;
    written_assets.push(name.to_string());
    Ok(())
}

fn collect_files_sorted(path: &Path) -> Result<Vec<PathBuf>, BackendError> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), BackendError> {
    let mut entries = fs::read_dir(path)
        .map_err(|error| BackendError::new(format!("failed to read {}: {error}", path.display())))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            BackendError::new(format!("failed to read {}: {error}", path.display()))
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let metadata = entry.metadata().map_err(|error| {
            BackendError::new(format!("failed to stat {}: {error}", path.display()))
        })?;
        if metadata.is_dir() {
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }

    Ok(())
}

fn board_asset_candidates(path: &Path) -> Vec<PathBuf> {
    let roots = board_asset_roots(path);
    let mut candidates = Vec::new();

    for root in roots {
        push_candidate(&mut candidates, root.join("et01.ic652"));
        push_candidate(&mut candidates, root.join("et03"));
        push_candidate(&mut candidates, root.join("at28c16_world"));
        push_candidate(&mut candidates, root.join("at28c16_usa"));
        push_candidate(&mut candidates, root.join("at28c16_japan"));
        push_candidate(&mut candidates, root.join("at28c16_asia"));
        push_candidate(&mut candidates, root.join("bldyror2.cfg"));
        push_candidate(&mut candidates, root.join("ZiNc.exe"));
        push_candidate(&mut candidates, root.join("cfg/bldyror2.cfg"));
        push_candidate(&mut candidates, root.join("extracted/BloodRoar2/ZiNc.exe"));
        push_candidate(
            &mut candidates,
            root.join("extracted/BloodRoar2/cfg/bldyror2.cfg"),
        );
    }

    candidates
}

fn board_asset_roots(path: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if path.is_dir() {
        roots.push(path.to_path_buf());
    }
    if let Some(parent) = path.parent() {
        roots.push(parent.to_path_buf());
        if let Some(grandparent) = parent.parent() {
            roots.push(grandparent.to_path_buf());
        }
    }
    roots
}

fn push_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() && !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn exact_8_bytes(bytes: &[u8]) -> Option<[u8; 8]> {
    let bytes: [u8; 8] = bytes.try_into().ok()?;
    Some(bytes)
}

fn find_crc32_window_8(bytes: &[u8], expected_crc32: u32) -> Option<[u8; 8]> {
    bytes
        .windows(8)
        .find(|window| crc32(window) == expected_crc32)
        .and_then(exact_8_bytes)
}

fn at28c16_fallback_bytes(path: &Path, bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() != 2048 {
        return None;
    }

    let crc = crc32(bytes);
    let is_known_region_eeprom = matches!(
        crc,
        AT28C16_WORLD_CRC32 | AT28C16_USA_CRC32 | AT28C16_JAPAN_CRC32 | AT28C16_ASIA_CRC32
    );
    let is_named_cfg = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("bldyror2.cfg")
                || name.eq_ignore_ascii_case("at28c16_world")
                || name.eq_ignore_ascii_case("at28c16_usa")
                || name.eq_ignore_ascii_case("at28c16_japan")
                || name.eq_ignore_ascii_case("at28c16_asia")
        });

    if is_known_region_eeprom || is_named_cfg {
        return Some(bytes.to_vec());
    }

    None
}

fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

fn is_zip_entry_name(name: &str) -> bool {
    !name.ends_with('/') && normalized_asset_name(name).ends_with(".zip")
}

fn manifest_match_for_entry(entry: &NativeRomEntry) -> NativeRomAssetMatch {
    let Some(manifest_entry) = manifest_entry_for_asset_name(&entry.name) else {
        return NativeRomAssetMatch {
            provided_name: entry.name.clone(),
            manifest_name: None,
            asset_group: "unknown",
            source_set: None,
            role: None,
            expected_size: None,
            actual_size: entry.uncompressed_size,
            expected_crc32: None,
            actual_crc32: entry.crc32,
            status: "unknown",
            issues: vec![
                "asset is not listed in the Bloody Roar 2 or Sony ZN BIOS manifest".to_string(),
            ],
        };
    };

    let mut issues = Vec::new();
    if entry.uncompressed_size != manifest_entry.expected_size {
        issues.push(format!(
            "size mismatch: expected {}, got {}",
            manifest_entry.expected_size, entry.uncompressed_size
        ));
    }
    if manifest_entry
        .expected_crc32
        .is_some_and(|expected_crc32| entry.crc32 != expected_crc32)
    {
        issues.push(format!(
            "crc32 mismatch: expected {:08x}, got {:08x}",
            manifest_entry.expected_crc32.expect("checked Some"),
            entry.crc32
        ));
    }

    NativeRomAssetMatch {
        provided_name: entry.name.clone(),
        manifest_name: Some(manifest_entry.name),
        asset_group: manifest_asset_group(manifest_entry),
        source_set: Some(manifest_entry.source_set),
        role: Some(manifest_entry.role),
        expected_size: Some(manifest_entry.expected_size),
        actual_size: entry.uncompressed_size,
        expected_crc32: manifest_entry.expected_crc32,
        actual_crc32: entry.crc32,
        status: if issues.is_empty() {
            "matched"
        } else {
            "mismatched"
        },
        issues,
    }
}

fn manifest_entry_for_asset_name(name: &str) -> Option<&'static NativeRomManifestEntry> {
    BLOODY_ROAR_2_MANIFEST
        .all_assets()
        .find(|entry| asset_names_match(name, entry.name))
}

fn manifest_asset_group(entry: &NativeRomManifestEntry) -> &'static str {
    if BLOODY_ROAR_2_MANIFEST
        .bios_assets
        .iter()
        .any(|bios_entry| bios_entry.name.eq_ignore_ascii_case(entry.name))
    {
        "bios"
    } else {
        "game"
    }
}

fn entry_mismatches_manifest(
    entry: &NativeRomEntry,
    manifest_entry: &NativeRomManifestEntry,
) -> bool {
    entry.uncompressed_size != manifest_entry.expected_size
        || manifest_entry
            .expected_crc32
            .is_some_and(|expected_crc32| entry.crc32 != expected_crc32)
}

fn asset_names_match(provided_name: &str, manifest_name: &str) -> bool {
    let provided = normalized_file_name(provided_name);
    let manifest = normalized_file_name(manifest_name);
    provided == manifest
        || ROM_NAME_ALIASES.iter().any(|(alias, canonical)| {
            (provided == *alias && manifest == *canonical)
                || (provided == *canonical && manifest == *alias)
        })
}

fn duplicate_asset_key(name: &str) -> String {
    if manifest_entry_for_asset_name(name).is_some() {
        normalized_file_name(name)
    } else {
        normalized_asset_name(name)
    }
}

fn normalized_asset_name(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

fn normalized_file_name(name: &str) -> String {
    normalized_asset_name(name)
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

fn is_required_asset_name(name: &str) -> bool {
    BLOODY_ROAR_2_REQUIRED_ASSETS
        .iter()
        .any(|expectation| asset_names_match(name, expectation.name))
}

fn parse_manifest_offset(offset: &str) -> Result<usize, BackendError> {
    usize::from_str_radix(offset, 16)
        .map_err(|error| BackendError::new(format!("invalid manifest offset {offset}: {error}")))
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn parse_zip_entries(bytes: &[u8]) -> Result<Vec<NativeRomEntry>, BackendError> {
    let eocd_offset = find_eocd(bytes).ok_or_else(|| {
        BackendError::new("failed to inspect ZIP archive: end of central directory not found")
    })?;

    let disk_number = read_u16(bytes, eocd_offset + 4)?;
    let central_directory_disk = read_u16(bytes, eocd_offset + 6)?;
    if disk_number != 0 || central_directory_disk != 0 {
        return Err(BackendError::new(
            "multi-disk ZIP archives are not supported by native inspection",
        ));
    }

    let entry_count = read_u16(bytes, eocd_offset + 10)? as usize;
    let central_directory_size = read_u32(bytes, eocd_offset + 12)? as usize;
    let central_directory_offset = read_u32(bytes, eocd_offset + 16)? as usize;
    if central_directory_offset
        .checked_add(central_directory_size)
        .is_none_or(|end| end > bytes.len())
    {
        return Err(BackendError::new(
            "ZIP central directory points outside the archive",
        ));
    }

    let mut entries = Vec::with_capacity(entry_count);
    let mut offset = central_directory_offset;
    let central_directory_end = central_directory_offset + central_directory_size;
    for _ in 0..entry_count {
        if offset >= central_directory_end {
            return Err(BackendError::new(
                "ZIP central directory ended before all entries were read",
            ));
        }
        let (entry, next_offset) = parse_central_directory_entry(bytes, offset)?;
        entries.push(entry);
        offset = next_offset;
    }

    Ok(entries)
}

fn parse_zip_entries_with_nested_archives(
    path: &Path,
    bytes: &[u8],
) -> Result<Vec<NativeRomEntry>, BackendError> {
    let entries = parse_zip_entries(bytes)?;
    let mut expanded = entries.clone();

    for entry in entries {
        if !is_zip_entry_name(&entry.name) {
            continue;
        }

        let Ok(nested_bytes) = read_zip_entry_from_bytes(path, bytes, &entry) else {
            continue;
        };
        let Ok(nested_entries) = parse_zip_entries(&nested_bytes) else {
            continue;
        };

        for mut nested_entry in nested_entries {
            if nested_entry.name.ends_with('/') {
                continue;
            }
            nested_entry.name = format!("{}/{}", entry.name, nested_entry.name);
            expanded.push(nested_entry);
        }
    }

    Ok(expanded)
}

fn find_eocd(bytes: &[u8]) -> Option<usize> {
    if bytes.len() < 22 {
        return None;
    }

    let min_offset = bytes.len().saturating_sub(22 + u16::MAX as usize);
    for offset in (min_offset..=bytes.len() - 22).rev() {
        if read_u32_at(bytes, offset)? != EOCD_SIGNATURE {
            continue;
        }

        let comment_len = read_u16_at(bytes, offset + 20)? as usize;
        if offset + 22 + comment_len == bytes.len() {
            return Some(offset);
        }
    }
    None
}

fn parse_central_directory_entry(
    bytes: &[u8],
    offset: usize,
) -> Result<(NativeRomEntry, usize), BackendError> {
    if read_u32(bytes, offset)? != CENTRAL_DIRECTORY_FILE_HEADER_SIGNATURE {
        return Err(BackendError::new(format!(
            "invalid ZIP central directory header at byte {offset}"
        )));
    }

    let compression_method = read_u16(bytes, offset + 10)?;
    let crc32 = read_u32(bytes, offset + 16)?;
    let compressed_size_32 = read_u32(bytes, offset + 20)?;
    let uncompressed_size_32 = read_u32(bytes, offset + 24)?;
    let file_name_len = read_u16(bytes, offset + 28)? as usize;
    let extra_len = read_u16(bytes, offset + 30)? as usize;
    let comment_len = read_u16(bytes, offset + 32)? as usize;
    let local_header_offset_32 = read_u32(bytes, offset + 42)?;
    let name_start = offset + 46;
    let extra_start = name_start
        .checked_add(file_name_len)
        .ok_or_else(|| BackendError::new("ZIP file name length overflow"))?;
    let comment_start = extra_start
        .checked_add(extra_len)
        .ok_or_else(|| BackendError::new("ZIP extra field length overflow"))?;
    let next_offset = comment_start
        .checked_add(comment_len)
        .ok_or_else(|| BackendError::new("ZIP comment length overflow"))?;

    if next_offset > bytes.len() {
        return Err(BackendError::new(
            "ZIP central directory entry points outside the archive",
        ));
    }

    let name = String::from_utf8_lossy(&bytes[name_start..extra_start]).to_string();
    let (uncompressed_size, compressed_size, local_header_offset) = zip64_entry_metadata(
        &bytes[extra_start..comment_start],
        uncompressed_size_32,
        compressed_size_32,
        local_header_offset_32,
    )?;

    Ok((
        NativeRomEntry {
            name,
            uncompressed_size,
            compressed_size,
            crc32,
            compression_method,
            local_header_offset: Some(local_header_offset),
        },
        next_offset,
    ))
}

fn zip64_entry_metadata(
    extra: &[u8],
    uncompressed_size_32: u32,
    compressed_size_32: u32,
    local_header_offset_32: u32,
) -> Result<(u64, u64, u64), BackendError> {
    let needs_uncompressed = uncompressed_size_32 == u32::MAX;
    let needs_compressed = compressed_size_32 == u32::MAX;
    let needs_local_header_offset = local_header_offset_32 == u32::MAX;
    if !needs_uncompressed && !needs_compressed && !needs_local_header_offset {
        return Ok((
            uncompressed_size_32 as u64,
            compressed_size_32 as u64,
            local_header_offset_32 as u64,
        ));
    }

    let mut offset = 0;
    while offset + 4 <= extra.len() {
        let header_id = read_u16(extra, offset)?;
        let data_size = read_u16(extra, offset + 2)? as usize;
        let data_start = offset + 4;
        let data_end = data_start
            .checked_add(data_size)
            .ok_or_else(|| BackendError::new("ZIP64 extra field length overflow"))?;
        if data_end > extra.len() {
            return Err(BackendError::new("ZIP64 extra field is truncated"));
        }

        if header_id == ZIP64_EXTENDED_INFORMATION_EXTRA_FIELD {
            let mut data_offset = data_start;
            let uncompressed_size = if needs_uncompressed {
                let value = read_u64(extra, data_offset)?;
                data_offset += 8;
                value
            } else {
                uncompressed_size_32 as u64
            };
            let compressed_size = if needs_compressed {
                let value = read_u64(extra, data_offset)?;
                data_offset += 8;
                value
            } else {
                compressed_size_32 as u64
            };
            let local_header_offset = if needs_local_header_offset {
                read_u64(extra, data_offset)?
            } else {
                local_header_offset_32 as u64
            };
            return Ok((uncompressed_size, compressed_size, local_header_offset));
        }

        offset = data_end;
    }

    Err(BackendError::new(
        "ZIP64 entry uses 32-bit placeholders without ZIP64 metadata",
    ))
}

fn read_zip_entry(path: &Path, entry: &NativeRomEntry) -> Result<Vec<u8>, BackendError> {
    let archive = fs::read(path).map_err(|error| {
        BackendError::new(format!(
            "failed to read ZIP archive {}: {error}",
            path.display()
        ))
    })?;
    read_zip_entry_from_bytes(path, &archive, entry)
}

fn read_zip_entry_from_bytes(
    archive_path: &Path,
    archive: &[u8],
    entry: &NativeRomEntry,
) -> Result<Vec<u8>, BackendError> {
    let compressed_data = zip_entry_compressed_data(archive_path, archive, entry)?;
    let expected_len = usize::try_from(entry.uncompressed_size).map_err(|_| {
        BackendError::new(format!(
            "ZIP entry {} in {} is too large to address on this platform",
            entry.name,
            archive_path.display()
        ))
    })?;

    let data = match entry.compression_method {
        ZIP_STORED_METHOD => {
            if entry.compressed_size != entry.uncompressed_size {
                return Err(BackendError::new(format!(
                    "stored ZIP entry {} in {} has mismatched compressed/uncompressed sizes",
                    entry.name,
                    archive_path.display()
                )));
            }
            compressed_data.to_vec()
        }
        ZIP_DEFLATED_METHOD => {
            inflate_raw_deflate(compressed_data, expected_len).map_err(|error| {
                BackendError::new(format!(
                    "failed to inflate ZIP entry {} in {}: {error}",
                    entry.name,
                    archive_path.display()
                ))
            })?
        }
        _ => {
            return Err(BackendError::new(format!(
                "ZIP entry {} in {} uses {}; native ROM loading supports stored and deflated entries without per-run extraction",
                entry.name,
                archive_path.display(),
                entry.compression_method_name()
            )));
        }
    };

    if data.len() != expected_len {
        return Err(BackendError::new(format!(
            "ZIP entry {} in {} decoded to {} bytes, expected {}",
            entry.name,
            archive_path.display(),
            data.len(),
            expected_len
        )));
    }
    if crc32(&data) != entry.crc32 {
        return Err(BackendError::new(format!(
            "ZIP entry {} in {} failed CRC validation",
            entry.name,
            archive_path.display()
        )));
    }

    Ok(data)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn inflate_raw_deflate(compressed_data: &[u8], expected_len: usize) -> Result<Vec<u8>, String> {
    zlib_inflate_raw_deflate(compressed_data, expected_len)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn inflate_raw_deflate(_compressed_data: &[u8], _expected_len: usize) -> Result<Vec<u8>, String> {
    Err("deflated ZIP entries require zlib support on this platform".to_string())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn zlib_inflate_raw_deflate(
    compressed_data: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, String> {
    if compressed_data.len() > c_uint::MAX as usize {
        return Err("compressed stream is too large for zlib".to_string());
    }
    if expected_len > c_uint::MAX as usize {
        return Err("decoded stream is too large for zlib".to_string());
    }

    let mut output = vec![0; expected_len];
    let mut stream = ZStream {
        next_in: compressed_data.as_ptr(),
        avail_in: compressed_data.len() as c_uint,
        total_in: 0,
        next_out: output.as_mut_ptr(),
        avail_out: output.len() as c_uint,
        total_out: 0,
        msg: std::ptr::null_mut(),
        state: std::ptr::null_mut(),
        zalloc: None,
        zfree: None,
        opaque: std::ptr::null_mut(),
        data_type: 0,
        adler: 0,
        reserved: 0,
    };

    // ZIP stores method 8 payloads as raw deflate streams without a zlib header.
    let init_status = unsafe {
        inflateInit2_(
            &mut stream,
            -MAX_WBITS,
            zlibVersion(),
            std::mem::size_of::<ZStream>() as c_int,
        )
    };
    if init_status != Z_OK {
        return Err(format_zlib_error("inflateInit2", init_status, &stream));
    }

    let inflate_status = unsafe { inflate(&mut stream, Z_NO_FLUSH) };
    let inflate_error = (inflate_status != Z_STREAM_END)
        .then(|| format_zlib_error("inflate", inflate_status, &stream));
    let end_status = unsafe { inflateEnd(&mut stream) };
    if let Some(error) = inflate_error {
        return Err(error);
    }
    if end_status != Z_OK {
        return Err(format_zlib_error("inflateEnd", end_status, &stream));
    }
    if stream.total_in as usize != compressed_data.len() {
        return Err(format!(
            "zlib consumed {} of {} compressed bytes",
            stream.total_in,
            compressed_data.len()
        ));
    }
    if stream.total_out as usize != expected_len {
        return Err(format!(
            "zlib produced {} of {expected_len} expected bytes",
            stream.total_out
        ));
    }

    Ok(output)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn format_zlib_error(operation: &str, status: c_int, stream: &ZStream) -> String {
    let message = if stream.msg.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(stream.msg) }
            .to_string_lossy()
            .into_owned()
    };
    if message.is_empty() {
        format!("{operation} returned zlib status {status}")
    } else {
        format!("{operation} returned zlib status {status}: {message}")
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
const Z_NO_FLUSH: c_int = 0;
#[cfg(any(target_os = "macos", target_os = "linux"))]
const Z_OK: c_int = 0;
#[cfg(any(target_os = "macos", target_os = "linux"))]
const Z_STREAM_END: c_int = 1;
#[cfg(any(target_os = "macos", target_os = "linux"))]
const MAX_WBITS: c_int = 15;

#[cfg(any(target_os = "macos", target_os = "linux"))]
type ZAllocFunc = Option<unsafe extern "C" fn(*mut c_void, c_uint, c_uint) -> *mut c_void>;
#[cfg(any(target_os = "macos", target_os = "linux"))]
type ZFreeFunc = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[repr(C)]
struct ZStream {
    next_in: *const u8,
    avail_in: c_uint,
    total_in: c_ulong,
    next_out: *mut u8,
    avail_out: c_uint,
    total_out: c_ulong,
    msg: *mut c_char,
    state: *mut c_void,
    zalloc: ZAllocFunc,
    zfree: ZFreeFunc,
    opaque: *mut c_void,
    data_type: c_int,
    adler: c_ulong,
    reserved: c_ulong,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[link(name = "z")]
unsafe extern "C" {
    fn zlibVersion() -> *const c_char;
    fn inflateInit2_(
        stream: *mut ZStream,
        window_bits: c_int,
        version: *const c_char,
        stream_size: c_int,
    ) -> c_int;
    fn inflate(stream: *mut ZStream, flush: c_int) -> c_int;
    fn inflateEnd(stream: *mut ZStream) -> c_int;
}

fn zip_entry_compressed_data<'a>(
    archive_path: &Path,
    archive: &'a [u8],
    entry: &NativeRomEntry,
) -> Result<&'a [u8], BackendError> {
    let data_start = zip_entry_data_start(archive_path, archive, entry)?;
    let data_len = usize::try_from(entry.compressed_size).map_err(|_| {
        BackendError::new(format!(
            "ZIP entry {} in {} is too large to address on this platform",
            entry.name,
            archive_path.display()
        ))
    })?;
    let data_end = data_start
        .checked_add(data_len)
        .ok_or_else(|| BackendError::new("ZIP entry data range overflow"))?;
    archive.get(data_start..data_end).ok_or_else(|| {
        BackendError::new(format!(
            "ZIP entry {} in {} points outside the archive",
            entry.name,
            archive_path.display()
        ))
    })
}

fn zip_entry_data_start(
    archive_path: &Path,
    archive: &[u8],
    entry: &NativeRomEntry,
) -> Result<usize, BackendError> {
    let local_header_offset = entry.local_header_offset.ok_or_else(|| {
        BackendError::new(format!(
            "ZIP entry {} in {} has no local header offset",
            entry.name,
            archive_path.display()
        ))
    })?;
    let offset = usize::try_from(local_header_offset).map_err(|_| {
        BackendError::new(format!(
            "ZIP entry {} local header offset is too large",
            entry.name
        ))
    })?;
    if read_u32(archive, offset)? != LOCAL_FILE_HEADER_SIGNATURE {
        return Err(BackendError::new(format!(
            "invalid ZIP local file header for {} in {}",
            entry.name,
            archive_path.display()
        )));
    }
    let name_len = read_u16(archive, offset + 26)? as usize;
    let extra_len = read_u16(archive, offset + 28)? as usize;
    offset
        .checked_add(30)
        .and_then(|value| value.checked_add(name_len))
        .and_then(|value| value.checked_add(extra_len))
        .ok_or_else(|| BackendError::new("ZIP local file header length overflow"))
}

fn read_nested_zip_entry(
    path: &Path,
    archive_entry: &str,
    inner_entry: &str,
) -> Result<Vec<u8>, BackendError> {
    let outer_archive = fs::read(path).map_err(|error| {
        BackendError::new(format!(
            "failed to read ZIP archive {}: {error}",
            path.display()
        ))
    })?;
    read_nested_zip_entry_from_bytes(path, &outer_archive, archive_entry, inner_entry)
}

fn read_nested_zip_entry_from_bytes(
    path: &Path,
    outer_archive: &[u8],
    archive_entry: &str,
    inner_entry: &str,
) -> Result<Vec<u8>, BackendError> {
    let outer_entries = parse_zip_entries(&outer_archive)?;
    let outer_entry = outer_entries
        .iter()
        .find(|entry| asset_names_match(&entry.name, archive_entry))
        .ok_or_else(|| {
            BackendError::new(format!(
                "nested archive {archive_entry} is missing from {}",
                path.display()
            ))
        })?;
    let nested_archive = read_zip_entry_from_bytes(path, outer_archive, outer_entry)?;
    let nested_entries = parse_zip_entries(&nested_archive)?;
    let nested_entry = nested_entries
        .iter()
        .find(|entry| asset_names_match(&entry.name, inner_entry))
        .ok_or_else(|| {
            BackendError::new(format!(
                "nested ZIP entry {inner_entry} is missing from {archive_entry}"
            ))
        })?;
    read_zip_entry_from_bytes(path, &nested_archive, nested_entry)
}

fn read_scanned_entry_bytes(root: &Path, entry_name: &str) -> Result<Vec<u8>, BackendError> {
    if let Some((archive_name, inner_entry)) = split_scanned_zip_entry(entry_name) {
        if let Some((nested_archive, nested_inner_entry)) = split_scanned_zip_entry(inner_entry) {
            return read_nested_zip_entry(
                &root.join(archive_name),
                nested_archive,
                nested_inner_entry,
            );
        }
        let archive_path = root.join(archive_name);
        let archive = fs::read(&archive_path).map_err(|error| {
            BackendError::new(format!(
                "failed to read ZIP archive {}: {error}",
                archive_path.display()
            ))
        })?;
        let entries = parse_zip_entries(&archive)?;
        let entry = entries
            .iter()
            .find(|entry| asset_names_match(&entry.name, inner_entry))
            .ok_or_else(|| {
                BackendError::new(format!(
                    "ZIP entry {inner_entry} is missing from {}",
                    archive_path.display()
                ))
            })?;
        return read_zip_entry_from_bytes(&archive_path, &archive, entry);
    }

    fs::read(root.join(entry_name)).map_err(|error| {
        BackendError::new(format!(
            "failed to read scanned ROM entry {}: {error}",
            root.join(entry_name).display()
        ))
    })
}

fn split_scanned_zip_entry(entry_name: &str) -> Option<(&str, &str)> {
    let (archive_name, inner_entry) = entry_name.split_once(".zip/")?;
    Some((entry_name.get(..archive_name.len() + 4)?, inner_entry))
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, BackendError> {
    read_u16_at(bytes, offset)
        .ok_or_else(|| BackendError::new(format!("unexpected end of ZIP at byte {offset}")))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, BackendError> {
    read_u32_at(bytes, offset)
        .ok_or_else(|| BackendError::new(format!("unexpected end of ZIP at byte {offset}")))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, BackendError> {
    let slice = bytes.get(offset..offset + 8).ok_or_else(|| {
        BackendError::new(format!("unexpected end of ZIP64 metadata at byte {offset}"))
    })?;
    Ok(u64::from_le_bytes(
        slice.try_into().expect("slice length checked"),
    ))
}

fn read_u16_at(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn read_u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn json_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn optional_crc_json(value: Option<u32>) -> String {
    value
        .map(|crc32| format!("\"{crc32:08x}\""))
        .unwrap_or_else(|| "null".to_string())
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|number| number.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn optional_str_json(value: Option<&str>) -> String {
    value
        .map(|text| format!("\"{}\"", escape_json(text)))
        .unwrap_or_else(|| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn inspect_enumerates_zip_entry_metadata() {
        let romset = inspect_fixture("metadata", fixture_zip());

        assert_eq!(
            romset.entries,
            vec!["coh-1002e.353".to_string(), "gfx/texture.bin".to_string()]
        );
        assert_eq!(
            romset.entry_metadata,
            vec![
                NativeRomEntry {
                    name: "coh-1002e.353".to_string(),
                    uncompressed_size: 4,
                    compressed_size: 4,
                    crc32: 0x1234_abcd,
                    compression_method: 0,
                    local_header_offset: Some(0),
                },
                NativeRomEntry {
                    name: "gfx/texture.bin".to_string(),
                    uncompressed_size: 128,
                    compressed_size: 13,
                    crc32: 0xfeed_beef,
                    compression_method: 8,
                    local_header_offset: Some(47),
                },
            ]
        );

        let json = romset.json();
        assert!(json.contains("\"entry_count\":2"));
        assert!(json.contains("\"crc32\":\"1234abcd\""));
        assert!(json.contains("\"compression_method_name\":\"deflated\""));
    }

    #[test]
    fn inspect_expands_and_loads_nested_zip_entries() {
        let inner_zip = fixture_stored_zip(&[("flash0.021", &[0x11, 0x22, 0x33, 0x44])]);
        let outer_zip = fixture_stored_zip(&[("BloodRoar2/roms/bldyror2.zip", &inner_zip)]);
        let zip_path = temp_zip_path("nested");
        fs::write(&zip_path, outer_zip).expect("write nested ZIP fixture");
        let romset = NativeRomSet::inspect(&zip_path).expect("inspect nested ZIP fixture");

        assert!(
            romset
                .entries
                .contains(&"BloodRoar2/roms/bldyror2.zip/flash0.021".to_string())
        );

        let bytes = romset
            .load_manifest_asset("flash0.021")
            .expect("load nested flash0 entry");
        assert_eq!(bytes, vec![0x11, 0x22, 0x33, 0x44]);
        let _ = fs::remove_file(zip_path);
    }

    #[test]
    fn scan_cached_materializes_nested_zip_assets_once_under_target_cache() {
        let inner_zip = fixture_stored_zip(&[("flash0.021", &[0x11, 0x22, 0x33, 0x44])]);
        let outer_zip = fixture_stored_zip(&[("BloodRoar2/roms/bldyror2.zip", &inner_zip)]);
        let zip_path = temp_zip_path("runtime-cache");
        fs::write(&zip_path, outer_zip).expect("write runtime cache ZIP fixture");

        let romset = NativeRomSet::scan_cached(&zip_path).expect("scan cached ZIP fixture");
        let cache_base = romset
            .path
            .parent()
            .expect("cache rom dir parent")
            .to_path_buf();
        assert!(romset.path.ends_with("roms"));
        assert!(
            romset
                .path
                .to_string_lossy()
                .contains("target/native-rom-cache")
        );
        assert_eq!(
            fs::read(romset.path.join("flash0.021")).expect("read cached flash0"),
            vec![0x11, 0x22, 0x33, 0x44]
        );
        assert!(!romset.entries.iter().any(|entry| entry == "READY"));

        let second = NativeRomSet::scan_cached(&zip_path).expect("reuse cached ZIP fixture");
        assert_eq!(second.path, romset.path);

        let _ = fs::remove_file(zip_path);
        let _ = fs::remove_dir_all(cache_base);
    }

    #[test]
    fn scan_cached_preserves_board_asset_fallbacks_from_source_neighbors() {
        let source_dir = temp_scan_dir("runtime-cache-board-assets");
        let zip_path = source_dir.join("game.zip");
        fs::write(
            &zip_path,
            fixture_stored_zip(&[("flash0.021", &[0x11, 0x22, 0x33, 0x44])]),
        )
        .expect("write runtime cache ZIP fixture");
        fs::write(source_dir.join("bldyror2.cfg"), vec![0x5a; 2048])
            .expect("write board asset fallback fixture");

        let romset = NativeRomSet::scan_cached(&zip_path)
            .expect("scan cached ZIP fixture with board fallback");
        let cache_base = romset
            .path
            .parent()
            .expect("cache rom dir parent")
            .to_path_buf();

        assert_eq!(
            fs::read(romset.path.join("bldyror2.cfg")).expect("read cached EEPROM fallback"),
            vec![0x5a; 2048]
        );

        let _ = fs::remove_dir_all(source_dir);
        let _ = fs::remove_dir_all(cache_base);
    }

    #[test]
    fn read_zip_entry_inflates_deflated_data_without_external_unzip() {
        let data = b"hello deflated zip";
        let compressed_data = [
            0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0x48, 0x49, 0x4d, 0xcb, 0x49, 0x2c, 0x49, 0x4d,
            0x51, 0xa8, 0xca, 0x2c, 0x00, 0x00,
        ];
        let zip = fixture_single_entry_zip(
            "deflated.txt",
            ZIP_DEFLATED_METHOD,
            crc32(data),
            data.len() as u32,
            &compressed_data,
        );
        let entries = parse_zip_entries(&zip).expect("parse deflated fixture");
        let bytes = read_zip_entry_from_bytes(Path::new("fixture.zip"), &zip, &entries[0])
            .expect("inflate deflated fixture");

        assert_eq!(bytes, data);
    }

    #[test]
    fn inspect_expands_deflated_nested_zip_entries() {
        let inner_zip = fixture_stored_zip(&[("flash0.021", &[0x11, 0x22, 0x33, 0x44])]);
        assert_eq!(inner_zip.len(), 122);
        assert_eq!(crc32(&inner_zip), 0x35a1_475c);
        let deflated_inner_zip = [
            0x0b, 0xf0, 0x66, 0x66, 0x11, 0x61, 0x80, 0x81, 0x8b, 0x73, 0x3f, 0x95, 0xb3, 0x00,
            0x69, 0x10, 0xe6, 0x02, 0xe2, 0xb4, 0x9c, 0xc4, 0xe2, 0x0c, 0x03, 0x3d, 0x03, 0x23,
            0x43, 0x41, 0x25, 0x63, 0x97, 0x00, 0x6f, 0x46, 0x26, 0x11, 0x06, 0xdc, 0xaa, 0x51,
            0x01, 0x42, 0x6f, 0x80, 0x37, 0x2b, 0x1b, 0x48, 0x84, 0x11, 0x08, 0x2d, 0x80, 0xb4,
            0x0e, 0x58, 0x1e, 0x00,
        ];
        let outer_zip = fixture_single_entry_zip(
            "BloodRoar2/roms/bldyror2.zip",
            ZIP_DEFLATED_METHOD,
            crc32(&inner_zip),
            inner_zip.len() as u32,
            &deflated_inner_zip,
        );
        let zip_path = temp_zip_path("deflated-nested");
        fs::write(&zip_path, outer_zip).expect("write deflated nested ZIP fixture");
        let romset = NativeRomSet::inspect(&zip_path).expect("inspect deflated nested ZIP fixture");

        assert!(
            romset
                .entries
                .contains(&"BloodRoar2/roms/bldyror2.zip/flash0.021".to_string())
        );
        let bytes = romset
            .load_manifest_asset("flash0.021")
            .expect("load deflated nested flash0 entry");
        assert_eq!(bytes, vec![0x11, 0x22, 0x33, 0x44]);
        let _ = fs::remove_file(zip_path);
    }

    #[test]
    fn inspect_reports_compatible_required_assets_from_valid_zip_fixture() {
        let romset = inspect_fixture("valid-required-assets", fixture_required_assets_zip(None));
        let report = romset.bloody_roar_2_compatibility();

        assert!(report.compatible());
        assert_eq!(romset.entries.len(), BLOODY_ROAR_2_REQUIRED_ASSETS.len());
        assert_eq!(
            report.present_assets.len(),
            BLOODY_ROAR_2_REQUIRED_ASSETS.len()
        );
        assert!(report.missing_required_assets.is_empty());
        assert!(report.mismatched_assets.is_empty());
        assert!(report.duplicate_assets.is_empty());

        let json = romset.json();
        assert!(json.contains("\"compatible\":true"));
        assert!(json.contains("\"has_duplicate_assets\":false"));
        assert!(json.contains("\"missing_required_assets\":[]"));
    }

    #[test]
    fn manifest_defines_sony_zn_bios_and_bloody_roar_2_game_assets() {
        let manifest = &BLOODY_ROAR_2_MANIFEST;

        assert_eq!(manifest.game_id, "bldyror2");
        assert_eq!(manifest.title, "Bloody Roar 2 (World)");
        assert_eq!(manifest.bios_set, "coh1002e");
        assert_eq!(manifest.bios_assets.len(), 3);
        assert_eq!(manifest.game_assets.len(), 11);
        assert_eq!(manifest.all_assets().count(), 14);

        assert_eq!(
            manifest.bios_assets[0],
            NativeRomManifestEntry {
                name: "m27c402cz-54.ic353",
                role: "zn_boot_rom",
                source_set: "coh1002e",
                required: true,
                expected_size: 524_288,
                expected_crc32: Some(0x910f_3a8b),
                expected_sha1: Some("cd68532967a25f476a6d73473ec6b6f4df2e1689"),
                region: "maincpu:rom",
                offset: "0",
                dump_status: "good",
                merge: None,
            }
        );

        let nodump = manifest
            .bios_assets
            .iter()
            .find(|asset| asset.name == "78081g503.ic655")
            .expect("protection MCU manifest entry");
        assert!(nodump.required);
        assert_eq!(nodump.expected_crc32, None);
        assert_eq!(nodump.expected_sha1, None);
        assert_eq!(nodump.dump_status, "nodump");
        assert_eq!(nodump.region, "upd78081");

        let flash0 = manifest
            .game_assets
            .iter()
            .find(|asset| asset.name == "flash0.021")
            .expect("flash0 manifest entry");
        assert_eq!(flash0.expected_size, 2_097_152);
        assert_eq!(flash0.expected_crc32, Some(0xfa76_02e1));
        assert_eq!(
            flash0.expected_sha1,
            Some("6fb6af09656fbb86d2abda35804b2ed4a4cd7461")
        );
        assert_eq!(flash0.region, "bankedroms");
        assert_eq!(flash0.offset, "0");
    }

    #[test]
    fn inspect_json_includes_manifest_metadata() {
        let romset = inspect_fixture("valid-manifest-json", fixture_required_assets_zip(None));
        let json = romset.json();

        assert!(json.contains("\"bloody_roar_2_manifest\""));
        assert!(json.contains("\"bios_set\":\"coh1002e\""));
        assert!(json.contains("\"expected_sha1\":\"cd68532967a25f476a6d73473ec6b6f4df2e1689\""));
        assert!(json.contains("\"region\":\"maincpu:rom\""));
        assert!(json.contains("\"dump_status\":\"nodump\""));
        assert!(json.contains("\"optional\":false"));
    }

    #[test]
    fn inspect_reports_missing_assets_from_zip_fixture() {
        let missing_asset = BLOODY_ROAR_2_REQUIRED_ASSETS[0].name;
        let romset = inspect_fixture(
            "missing-required-asset",
            fixture_required_assets_zip(Some(missing_asset)),
        );
        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert_eq!(
            romset.entries.len(),
            BLOODY_ROAR_2_REQUIRED_ASSETS.len() - 1
        );
        assert_eq!(
            report.missing_required_assets,
            vec![missing_asset.to_string()]
        );
        assert!(report.mismatched_assets.is_empty());
        assert!(report.duplicate_assets.is_empty());

        let json = romset.json();
        assert!(json.contains("\"compatible\":false"));
        assert!(json.contains("\"missing_required_assets\":[\"m27c402cz-54.ic353\"]"));
    }

    #[test]
    fn scan_directory_maps_bios_game_unknown_and_mismatched_assets() {
        let scan_dir = temp_scan_dir("manifest-scan");
        let bios_dir = scan_dir.join("coh1002e");
        fs::create_dir_all(&bios_dir).expect("create BIOS fixture dir");
        fs::write(bios_dir.join("m27c402cz-54.ic353"), [1, 2, 3, 4])
            .expect("write fake BIOS fixture");

        let zip_path = scan_dir.join("bldyror2.zip");
        fs::write(
            &zip_path,
            fixture_partial_manifest_zip(&[
                ("flash0.021", 0xfa76_02e1, 1, 2_097_152),
                ("unknown-extra.bin", 0x1234_5678, 4, 4),
            ]),
        )
        .expect("write fake ROM ZIP fixture");

        let romset = NativeRomSet::scan(&scan_dir).expect("scan directory fixture");
        let report = romset.bloody_roar_2_compatibility();
        let _ = fs::remove_dir_all(&scan_dir);

        assert!(!report.compatible());
        assert_eq!(
            report.present_bios_assets,
            vec!["m27c402cz-54.ic353".to_string()]
        );
        assert_eq!(report.present_game_assets, vec!["flash0.021".to_string()]);
        assert!(
            report
                .unknown_assets
                .contains(&"bldyror2.zip/unknown-extra.bin".to_string())
        );
        assert!(
            report
                .missing_required_assets
                .contains(&"et01.ic652".to_string())
        );
        assert_eq!(report.mismatched_assets.len(), 1);
        assert_eq!(report.mismatched_assets[0].name, "m27c402cz-54.ic353");

        let flash_match = report
            .asset_matches
            .iter()
            .find(|asset_match| asset_match.manifest_name == Some("flash0.021"))
            .expect("flash0 manifest match");
        assert_eq!(flash_match.asset_group, "game");
        assert_eq!(flash_match.source_set, Some("bldyror2"));
        assert_eq!(flash_match.status, "matched");

        let bios_match = report
            .asset_matches
            .iter()
            .find(|asset_match| asset_match.manifest_name == Some("m27c402cz-54.ic353"))
            .expect("BIOS manifest match");
        assert_eq!(bios_match.asset_group, "bios");
        assert_eq!(bios_match.source_set, Some("coh1002e"));
        assert_eq!(bios_match.status, "mismatched");

        let json = romset.json();
        assert!(json.contains("\"asset_group\":\"bios\""));
        assert!(json.contains("\"asset_group\":\"game\""));
        assert!(json.contains("\"unknown_assets\":[\"bldyror2.zip/unknown-extra.bin\"]"));
        assert!(json.contains("\"status\":\"mismatched\""));
    }

    #[test]
    fn inspect_reports_duplicate_zip_entries() {
        let romset = inspect_fixture("duplicate", fixture_duplicate_zip());

        assert_eq!(
            romset.duplicate_assets(),
            vec![NativeRomDuplicateAsset {
                name: "gfx/texture.bin".to_string(),
                normalized_name: "gfx/texture.bin".to_string(),
                occurrences: 2,
                entries: vec![
                    NativeRomEntry {
                        name: "gfx/texture.bin".to_string(),
                        uncompressed_size: 128,
                        compressed_size: 13,
                        crc32: 0xfeed_beef,
                        compression_method: 8,
                        local_header_offset: Some(47),
                    },
                    NativeRomEntry {
                        name: "GFX\\TEXTURE.BIN".to_string(),
                        uncompressed_size: 256,
                        compressed_size: 14,
                        crc32: 0xcafe_babe,
                        compression_method: 8,
                        local_header_offset: Some(105),
                    },
                ],
            }]
        );

        let json = romset.json();
        assert!(json.contains("\"has_duplicate_assets\":true"));
        assert!(json.contains("\"duplicate_asset_count\":1"));
        assert!(json.contains("\"normalized_name\":\"gfx/texture.bin\""));
        assert!(json.contains("\"occurrences\":2"));
    }

    #[test]
    fn inspect_flags_duplicate_required_assets_from_zip_fixture() {
        let romset = inspect_fixture("duplicate-required-asset", fixture_duplicate_required_zip());
        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert!(report.has_duplicate_required_assets());
        assert!(report.missing_required_assets.is_empty());
        assert!(report.mismatched_assets.is_empty());
        assert_eq!(report.duplicate_assets.len(), 1);
        assert_eq!(report.duplicate_assets[0].normalized_name, "flash0.021");
        assert_eq!(report.duplicate_assets[0].occurrences, 2);

        let json = romset.json();
        assert!(json.contains("\"has_duplicate_assets\":true"));
        assert!(json.contains("\"has_duplicate_required_assets\":true"));
        assert!(json.contains("\"duplicate_asset_count\":1"));
    }

    #[test]
    fn inspect_preserves_mixed_compression_zip_fixture_metadata() {
        let romset = inspect_fixture("mixed-compression", fixture_mixed_compression_zip());
        let compression_methods = romset
            .entry_metadata
            .iter()
            .map(|entry| (entry.name.as_str(), entry.compression_method_name()))
            .collect::<Vec<_>>();

        assert_eq!(
            compression_methods,
            vec![
                ("stored.bin", "stored"),
                ("deflated.bin", "deflated"),
                ("bzip2.bin", "bzip2"),
                ("zstd.bin", "zstd"),
            ]
        );

        let json = romset.json();
        assert!(json.contains("\"compression_method_name\":\"stored\""));
        assert!(json.contains("\"compression_method_name\":\"deflated\""));
        assert!(json.contains("\"compression_method_name\":\"bzip2\""));
        assert!(json.contains("\"compression_method_name\":\"zstd\""));
    }

    #[test]
    fn inspect_rejects_non_zip_input() {
        let error = parse_zip_entries(b"not a zip").expect_err("non-ZIP input should fail");
        assert!(
            error
                .to_string()
                .contains("end of central directory not found")
        );
    }

    #[test]
    fn compatibility_report_lists_missing_required_assets() {
        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries: vec!["flash0.021".to_string()],
            entry_metadata: vec![NativeRomEntry {
                name: "flash0.021".to_string(),
                uncompressed_size: 2_097_152,
                compressed_size: 2_097_152,
                crc32: 0xfa76_02e1,
                compression_method: 0,
                local_header_offset: None,
            }],
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };

        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert_eq!(report.present_assets, vec!["flash0.021".to_string()]);
        assert!(
            report
                .missing_required_assets
                .contains(&"m27c402cz-54.ic353".to_string())
        );
        assert!(
            report
                .missing_required_assets
                .contains(&"rom-3.336".to_string())
        );
        assert_eq!(report.missing_required_assets.len(), 13);

        let json = romset.json();
        assert!(json.contains("\"bloody_roar_2_compatibility\""));
        assert!(json.contains("\"missing_required_assets\":[\"m27c402cz-54.ic353\""));
    }

    #[test]
    fn compatibility_report_flags_present_assets_with_wrong_metadata() {
        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries: vec!["flash0.021".to_string()],
            entry_metadata: vec![NativeRomEntry {
                name: "flash0.021".to_string(),
                uncompressed_size: 4,
                compressed_size: 4,
                crc32: 0x1234_5678,
                compression_method: 0,
                local_header_offset: None,
            }],
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };

        let report = romset.bloody_roar_2_compatibility();

        assert_eq!(
            report.mismatched_assets,
            vec![NativeRomAssetMismatch {
                name: "flash0.021".to_string(),
                role: "program_flash",
                expected_size: 2_097_152,
                actual_size: 4,
                expected_crc32: Some(0xfa76_02e1),
                actual_crc32: 0x1234_5678,
            }]
        );
        assert!(report.json().contains("\"expected_crc32\":\"fa7602e1\""));
        assert!(report.json().contains("\"actual_crc32\":\"12345678\""));
    }

    #[test]
    fn compatibility_report_flags_duplicate_required_assets() {
        let mut entries = Vec::new();
        let mut entry_metadata = Vec::new();

        for expectation in BLOODY_ROAR_2_REQUIRED_ASSETS {
            entries.push(expectation.name.to_string());
            entry_metadata.push(NativeRomEntry {
                name: expectation.name.to_string(),
                uncompressed_size: expectation.expected_size,
                compressed_size: expectation.expected_size,
                crc32: expectation.expected_crc32.unwrap_or(0),
                compression_method: 0,
                local_header_offset: None,
            });
        }
        entries.push("FLASH0.021".to_string());
        entry_metadata.push(NativeRomEntry {
            name: "FLASH0.021".to_string(),
            uncompressed_size: 2_097_152,
            compressed_size: 2_097_152,
            crc32: 0xfa76_02e1,
            compression_method: 0,
            local_header_offset: None,
        });

        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries,
            entry_metadata,
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };
        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert!(report.has_duplicate_required_assets());
        assert!(report.missing_required_assets.is_empty());
        assert!(report.mismatched_assets.is_empty());
        assert_eq!(report.duplicate_assets.len(), 1);
        assert_eq!(report.duplicate_assets[0].normalized_name, "flash0.021");

        let json = report.json();
        assert!(json.contains("\"has_duplicate_required_assets\":true"));
        assert!(json.contains("\"duplicate_assets\":[{\"name\":\"flash0.021\""));
    }

    #[test]
    fn compatibility_report_passes_when_all_required_assets_match() {
        let mut entries = Vec::new();
        let mut entry_metadata = Vec::new();

        for expectation in BLOODY_ROAR_2_REQUIRED_ASSETS {
            entries.push(expectation.name.to_string());
            entry_metadata.push(NativeRomEntry {
                name: expectation.name.to_string(),
                uncompressed_size: expectation.expected_size,
                compressed_size: expectation.expected_size,
                crc32: expectation.expected_crc32.unwrap_or(0),
                compression_method: 0,
                local_header_offset: None,
            });
        }

        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries,
            entry_metadata,
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };

        let report = romset.bloody_roar_2_compatibility();

        assert!(report.compatible());
        assert_eq!(report.present_assets.len(), 14);
        assert!(report.missing_required_assets.is_empty());
        assert!(report.mismatched_assets.is_empty());
    }

    #[test]
    fn native_runtime_accepts_zinc_jp_flash_variant_without_security_dumps() {
        let mut entries = Vec::new();
        let mut entry_metadata = Vec::new();

        for expectation in BLOODY_ROAR_2_REQUIRED_ASSETS {
            if matches!(
                expectation.name,
                "et01.ic652" | "78081g503.ic655" | "at28c16_world" | "et03"
            ) {
                continue;
            }
            let crc32 = if expectation.name == "flash1.024" {
                ZINC_JP_FLASH1_CRC32
            } else {
                expectation.expected_crc32.unwrap_or(0)
            };
            entries.push(expectation.name.to_string());
            entry_metadata.push(NativeRomEntry {
                name: expectation.name.to_string(),
                uncompressed_size: expectation.expected_size,
                compressed_size: expectation.expected_size,
                crc32,
                compression_method: 0,
                local_header_offset: None,
            });
        }

        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries,
            entry_metadata,
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };

        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert!(report.native_runtime_usable());
        assert_eq!(
            report.known_variants(),
            vec!["zinc_jp_bundle_flash_variant"]
        );
        assert!(
            report
                .summary_json()
                .contains("\"native_runtime_usable\":true")
        );
    }

    #[test]
    fn native_runtime_rejects_missing_program_flash() {
        let romset = inspect_fixture(
            "missing-runtime-required-asset",
            fixture_required_assets_zip(Some("flash0.021")),
        );
        let report = romset.bloody_roar_2_compatibility();

        assert!(!report.compatible());
        assert!(!report.native_runtime_usable());
        assert!(
            report
                .summary_json()
                .contains("\"native_runtime_usable\":false")
        );
    }

    #[test]
    fn compatibility_treats_coh1002e_filename_as_bios_alias() {
        let romset = NativeRomSet {
            path: PathBuf::from("fixture.zip"),
            entries: vec!["coh-1002e.353".to_string()],
            entry_metadata: vec![NativeRomEntry {
                name: "coh-1002e.353".to_string(),
                uncompressed_size: 524_288,
                compressed_size: 524_288,
                crc32: 0x910f_3a8b,
                compression_method: 0,
                local_header_offset: None,
            }],
            archive_bytes: None,
            entry_bytes_cache: RefCell::new(BTreeMap::new()),
        };

        let report = romset.bloody_roar_2_compatibility();

        assert!(
            report
                .present_bios_assets
                .contains(&"m27c402cz-54.ic353".to_string())
        );
        assert!(!report.unknown_assets.contains(&"coh-1002e.353".to_string()));
        assert!(
            report
                .missing_required_assets
                .contains(&"flash0.021".to_string())
        );
    }

    #[test]
    fn load_banked_roms_places_assets_at_manifest_offsets() {
        let scan_dir = temp_scan_dir("banked-rom-load");
        fs::write(scan_dir.join("flash0.021"), [0x11, 0x22]).expect("write flash0");
        fs::write(scan_dir.join("rom-1a.028"), [0xaa, 0xbb]).expect("write rom-1a");

        let romset = NativeRomSet::scan(&scan_dir).expect("scan banked ROM fixture");
        let banked_roms = romset.load_banked_roms().expect("load banked ROM fixture");
        let _ = fs::remove_dir_all(&scan_dir);

        assert_eq!(&banked_roms[0..2], &[0x11, 0x22]);
        assert_eq!(&banked_roms[0x80_0000..0x80_0002], &[0xaa, 0xbb]);
    }

    fn inspect_fixture(name: &str, bytes: Vec<u8>) -> NativeRomSet {
        let zip_path = temp_zip_path(name);
        fs::write(&zip_path, bytes).expect("write test ZIP");

        let romset = NativeRomSet::inspect(&zip_path).expect("inspect test ZIP");
        let _ = fs::remove_file(&zip_path);
        romset
    }

    fn temp_zip_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "bloodyroar2-native-romset-{name}-{}.zip",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ))
    }

    fn temp_scan_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "bloodyroar2-native-romset-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock before unix epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&path).expect("create temp scan dir");
        path
    }

    fn fixture_zip() -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        push_entry(
            &mut zip,
            &mut central_directory,
            "coh-1002e.353",
            0,
            0x1234_abcd,
            4,
            4,
            &[1, 2, 3, 4],
        );
        push_entry(
            &mut zip,
            &mut central_directory,
            "gfx/texture.bin",
            8,
            0xfeed_beef,
            13,
            128,
            b"deflated-data",
        );

        finish_zip(zip, central_directory, 2)
    }

    fn fixture_duplicate_zip() -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        push_entry(
            &mut zip,
            &mut central_directory,
            "coh-1002e.353",
            0,
            0x1234_abcd,
            4,
            4,
            &[1, 2, 3, 4],
        );
        push_entry(
            &mut zip,
            &mut central_directory,
            "gfx/texture.bin",
            8,
            0xfeed_beef,
            13,
            128,
            b"deflated-data",
        );
        push_entry(
            &mut zip,
            &mut central_directory,
            "GFX\\TEXTURE.BIN",
            8,
            0xcafe_babe,
            14,
            256,
            b"deflated-data2",
        );

        finish_zip(zip, central_directory, 3)
    }

    fn fixture_required_assets_zip(skip_name: Option<&str>) -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        for expectation in BLOODY_ROAR_2_REQUIRED_ASSETS {
            if skip_name.is_some_and(|name| expectation.name.eq_ignore_ascii_case(name)) {
                continue;
            }

            push_entry(
                &mut zip,
                &mut central_directory,
                expectation.name,
                8,
                expectation.expected_crc32.unwrap_or(0),
                1,
                expectation.expected_size as u32,
                &[0],
            );
        }

        finish_zip(
            zip,
            central_directory,
            BLOODY_ROAR_2_REQUIRED_ASSETS.len() as u16 - u16::from(skip_name.is_some()),
        )
    }

    fn fixture_partial_manifest_zip(entries: &[(&str, u32, u32, u32)]) -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        for (name, crc32, compressed_size, uncompressed_size) in entries {
            push_entry(
                &mut zip,
                &mut central_directory,
                name,
                8,
                *crc32,
                *compressed_size,
                *uncompressed_size,
                &vec![0; *compressed_size as usize],
            );
        }

        finish_zip(zip, central_directory, entries.len() as u16)
    }

    fn fixture_duplicate_required_zip() -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        for expectation in BLOODY_ROAR_2_REQUIRED_ASSETS {
            push_entry(
                &mut zip,
                &mut central_directory,
                expectation.name,
                8,
                expectation.expected_crc32.unwrap_or(0),
                1,
                expectation.expected_size as u32,
                &[0],
            );
        }

        push_entry(
            &mut zip,
            &mut central_directory,
            "FLASH0.021",
            8,
            0xfa76_02e1,
            1,
            2_097_152,
            &[0],
        );

        finish_zip(
            zip,
            central_directory,
            BLOODY_ROAR_2_REQUIRED_ASSETS.len() as u16 + 1,
        )
    }

    fn fixture_mixed_compression_zip() -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        for (name, compression_method) in [
            ("stored.bin", 0),
            ("deflated.bin", 8),
            ("bzip2.bin", 12),
            ("zstd.bin", 93),
        ] {
            push_entry(
                &mut zip,
                &mut central_directory,
                name,
                compression_method,
                0x1111_0000 + compression_method as u32,
                1,
                1,
                &[compression_method as u8],
            );
        }

        finish_zip(zip, central_directory, 4)
    }

    fn fixture_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        for (name, data) in entries {
            push_entry(
                &mut zip,
                &mut central_directory,
                name,
                0,
                crc32(data),
                data.len() as u32,
                data.len() as u32,
                data,
            );
        }

        finish_zip(zip, central_directory, entries.len() as u16)
    }

    fn fixture_single_entry_zip(
        name: &str,
        compression_method: u16,
        crc32: u32,
        uncompressed_size: u32,
        compressed_data: &[u8],
    ) -> Vec<u8> {
        let mut zip = Vec::new();
        let mut central_directory = Vec::new();

        push_entry(
            &mut zip,
            &mut central_directory,
            name,
            compression_method,
            crc32,
            compressed_data.len() as u32,
            uncompressed_size,
            compressed_data,
        );

        finish_zip(zip, central_directory, 1)
    }

    fn finish_zip(mut zip: Vec<u8>, central_directory: Vec<u8>, entry_count: u16) -> Vec<u8> {
        let central_directory_offset = zip.len() as u32;
        let central_directory_size = central_directory.len() as u32;
        zip.extend_from_slice(&central_directory);
        zip.extend_from_slice(&EOCD_SIGNATURE.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&entry_count.to_le_bytes());
        zip.extend_from_slice(&entry_count.to_le_bytes());
        zip.extend_from_slice(&central_directory_size.to_le_bytes());
        zip.extend_from_slice(&central_directory_offset.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip
    }

    #[allow(clippy::too_many_arguments)]
    fn push_entry(
        zip: &mut Vec<u8>,
        central_directory: &mut Vec<u8>,
        name: &str,
        compression_method: u16,
        crc32: u32,
        compressed_size: u32,
        uncompressed_size: u32,
        data: &[u8],
    ) {
        let local_header_offset = zip.len() as u32;
        let name_bytes = name.as_bytes();

        zip.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        zip.extend_from_slice(&20u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&compression_method.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&crc32.to_le_bytes());
        zip.extend_from_slice(&compressed_size.to_le_bytes());
        zip.extend_from_slice(&uncompressed_size.to_le_bytes());
        zip.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(name_bytes);
        zip.extend_from_slice(data);

        central_directory.extend_from_slice(&CENTRAL_DIRECTORY_FILE_HEADER_SIGNATURE.to_le_bytes());
        central_directory.extend_from_slice(&20u16.to_le_bytes());
        central_directory.extend_from_slice(&20u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&compression_method.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&crc32.to_le_bytes());
        central_directory.extend_from_slice(&compressed_size.to_le_bytes());
        central_directory.extend_from_slice(&uncompressed_size.to_le_bytes());
        central_directory.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u16.to_le_bytes());
        central_directory.extend_from_slice(&0u32.to_le_bytes());
        central_directory.extend_from_slice(&local_header_offset.to_le_bytes());
        central_directory.extend_from_slice(name_bytes);
    }
}
