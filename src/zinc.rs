use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::backend::BackendError;

const UNSAFE_ZINC_WINE_ENV: &str = "BLOODYROAR2_ALLOW_UNSAFE_ZINC_WINE";

#[derive(Clone, Debug)]
pub struct ZincConfig {
    pub wine: PathBuf,
    pub bundle_dir: PathBuf,
    pub game_id: String,
    pub renderer: String,
    pub renderer_cfg: String,
}

impl Default for ZincConfig {
    fn default() -> Self {
        Self {
            wine: default_wine_path(),
            bundle_dir: PathBuf::from("assets/extracted/BloodRoar2"),
            game_id: "28".to_string(),
            renderer: "renderer-sft.znc".to_string(),
            renderer_cfg: "zenith-renderer70.cfg".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ZincRuntime {
    config: ZincConfig,
}

impl ZincRuntime {
    pub fn new(config: ZincConfig) -> Self {
        Self { config }
    }

    pub fn prepare_bundle(&self, archive: &Path, extract_dir: &Path) -> Result<(), BackendError> {
        fs::create_dir_all(extract_dir).map_err(|error| {
            BackendError::new(format!(
                "failed to create {}: {error}",
                extract_dir.display()
            ))
        })?;

        if extract_archive(archive, extract_dir).is_ok() {
            return Ok(());
        }

        let combined = extract_dir.join("zinc-bundle-combined.zip");
        let status = Command::new("zip")
            .arg("-s")
            .arg("0")
            .arg(archive)
            .arg("--out")
            .arg(&combined)
            .stdin(Stdio::null())
            .status()
            .map_err(|error| BackendError::new(format!("failed to run zip: {error}")))?;

        if !status.success() {
            return Err(BackendError::new(format!(
                "failed to combine split archive {}",
                archive.display()
            )));
        }

        extract_archive(&combined, extract_dir)
    }

    pub fn check(&self) -> String {
        let exe = self.config.bundle_dir.join("ZiNc.exe");
        let renderer = self.config.bundle_dir.join(&self.config.renderer);
        let renderer_cfg = self.config.bundle_dir.join(&self.config.renderer_cfg);
        let rom = self.config.bundle_dir.join("roms/bldyror2.zip");

        format!(
            "{{\"wine\":\"{}\",\"wine_found\":{},\"bundle_dir\":\"{}\",\"zinc_exe_found\":{},\"renderer\":\"{}\",\"renderer_found\":{},\"renderer_cfg\":\"{}\",\"renderer_cfg_found\":{},\"bldyror2_rom_found\":{},\"zinc_play_default_denied\":true,\"unsafe_opt_in_env\":\"{}\",\"unsafe_opt_in\":{},\"note\":\"zinc-play is default-denied because it launches a local Windows ZiNc executable through Wine. Set {}=1 only inside an isolated environment if you explicitly accept the host risk.\"}}",
            self.config.wine.display(),
            command_exists(&self.config.wine),
            self.config.bundle_dir.display(),
            exe.is_file(),
            self.config.renderer,
            renderer.is_file(),
            self.config.renderer_cfg,
            renderer_cfg.is_file(),
            rom.is_file(),
            UNSAFE_ZINC_WINE_ENV,
            unsafe_zinc_wine_opt_in_enabled(),
            UNSAFE_ZINC_WINE_ENV
        )
    }

    pub fn play(&self, extra_args: &[String]) -> Result<(), BackendError> {
        self.play_with_unsafe_opt_in(
            extra_args,
            std::env::var_os(UNSAFE_ZINC_WINE_ENV).as_deref(),
        )
    }

    fn play_with_unsafe_opt_in(
        &self,
        extra_args: &[String],
        unsafe_opt_in: Option<&OsStr>,
    ) -> Result<(), BackendError> {
        require_unsafe_zinc_wine_opt_in_from(unsafe_opt_in)?;
        self.ensure_ready()?;

        let renderer = format!("--renderer={}", self.config.renderer);
        let renderer_cfg = format!("--use-renderer-cfg-file={}", self.config.renderer_cfg);

        let mut args = vec![
            OsString::from("ZiNc.exe"),
            OsString::from(&self.config.game_id),
            OsString::from(renderer),
            OsString::from(renderer_cfg),
        ];
        args.extend(extra_args.iter().map(OsString::from));

        let status = Command::new(&self.config.wine)
            .args(args)
            .current_dir(&self.config.bundle_dir)
            .status()
            .map_err(|error| {
                BackendError::new(format!(
                    "failed to launch {}: {error}",
                    self.config.wine.display()
                ))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(BackendError::new(format!("ZiNc exited with {status}")))
        }
    }

    fn ensure_ready(&self) -> Result<(), BackendError> {
        if !command_exists(&self.config.wine) {
            return Err(BackendError::new(format!(
                "Wine executable not found: {}",
                self.config.wine.display()
            )));
        }

        let exe = self.config.bundle_dir.join("ZiNc.exe");
        if !exe.is_file() {
            return Err(BackendError::new(format!(
                "ZiNc.exe not found: {}",
                exe.display()
            )));
        }

        let renderer = self.config.bundle_dir.join(&self.config.renderer);
        if !renderer.is_file() {
            return Err(BackendError::new(format!(
                "renderer not found: {}",
                renderer.display()
            )));
        }

        let rom = self.config.bundle_dir.join("roms/bldyror2.zip");
        if !rom.is_file() {
            return Err(BackendError::new(format!(
                "bldyror2 ROM not found: {}",
                rom.display()
            )));
        }

        Ok(())
    }
}

fn require_unsafe_zinc_wine_opt_in_from(value: Option<&OsStr>) -> Result<(), BackendError> {
    if zinc_wine_opt_in_enabled_from(value) {
        Ok(())
    } else {
        Err(BackendError::new(format!(
            "zinc-play is disabled by default because it launches a local Windows ZiNc executable through Wine. Use the Rust-native or MAME path instead. To run this unsafe legacy path anyway, set {UNSAFE_ZINC_WINE_ENV}=1 only inside an isolated environment after scanning the local bundle and accepting the host risk."
        )))
    }
}

fn unsafe_zinc_wine_opt_in_enabled() -> bool {
    zinc_wine_opt_in_enabled_from(std::env::var_os(UNSAFE_ZINC_WINE_ENV).as_deref())
}

fn zinc_wine_opt_in_enabled_from(value: Option<&OsStr>) -> bool {
    value
        .and_then(OsStr::to_str)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

fn extract_archive(archive: &Path, extract_dir: &Path) -> Result<(), BackendError> {
    validate_archive_entries(archive)?;
    let status = Command::new("unzip")
        .arg("-o")
        .arg(archive)
        .arg("-d")
        .arg(extract_dir)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| BackendError::new(format!("failed to run unzip: {error}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(BackendError::new(format!(
            "failed to extract {}",
            archive.display()
        )))
    }
}

fn validate_archive_entries(archive: &Path) -> Result<(), BackendError> {
    let output = Command::new("unzip")
        .arg("-Z1")
        .arg(archive)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| BackendError::new(format!("failed to inspect archive: {error}")))?;
    if !output.status.success() {
        return Err(BackendError::new(format!(
            "failed to inspect archive entries in {}",
            archive.display()
        )));
    }

    for entry in String::from_utf8_lossy(&output.stdout).lines() {
        if !archive_entry_is_safe(entry) {
            return Err(BackendError::new(format!(
                "refusing unsafe archive entry {entry:?} in {}",
                archive.display()
            )));
        }
    }
    Ok(())
}

fn archive_entry_is_safe(entry: &str) -> bool {
    !entry.is_empty()
        && !entry.starts_with(['/', '\\'])
        && !entry.split(['/', '\\']).any(|component| component == "..")
}

fn default_wine_path() -> PathBuf {
    if let Some(value) = std::env::var_os("BLOODYROAR2_WINE") {
        return PathBuf::from(value);
    }

    let wine_stable =
        PathBuf::from("/Applications/Wine Stable.app/Contents/Resources/wine/bin/wine");
    if wine_stable.is_file() {
        return wine_stable;
    }

    PathBuf::from("wine")
}

fn command_exists(path: &Path) -> bool {
    if path.components().count() > 1 {
        return path.is_file();
    }

    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths)
                .map(|directory| directory.join(path))
                .any(|candidate| candidate.is_file())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{
        UNSAFE_ZINC_WINE_ENV, ZincConfig, ZincRuntime, archive_entry_is_safe,
        require_unsafe_zinc_wine_opt_in_from, zinc_wine_opt_in_enabled_from,
    };

    #[test]
    fn unsafe_zinc_wine_opt_in_is_explicit() {
        assert!(!zinc_wine_opt_in_enabled_from(None));
        assert!(!zinc_wine_opt_in_enabled_from(Some(OsStr::new(""))));
        assert!(!zinc_wine_opt_in_enabled_from(Some(OsStr::new("0"))));
        assert!(!zinc_wine_opt_in_enabled_from(Some(OsStr::new("false"))));
        assert!(!zinc_wine_opt_in_enabled_from(Some(OsStr::new("please"))));
        assert!(zinc_wine_opt_in_enabled_from(Some(OsStr::new("1"))));
        assert!(zinc_wine_opt_in_enabled_from(Some(OsStr::new("TRUE"))));
        assert!(zinc_wine_opt_in_enabled_from(Some(OsStr::new(" yes "))));
    }

    #[test]
    fn zinc_play_default_denial_names_required_env_and_risk() {
        let error = require_unsafe_zinc_wine_opt_in_from(None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("zinc-play is disabled by default"));
        assert!(error.contains(UNSAFE_ZINC_WINE_ENV));
        assert!(error.contains("Wine"));
        assert!(error.contains("unsafe"));
    }

    #[test]
    fn zinc_play_denies_before_readiness_checks_without_opt_in() {
        let runtime = ZincRuntime::new(ZincConfig {
            wine: std::path::PathBuf::from("missing-wine-for-test"),
            bundle_dir: std::path::PathBuf::from("missing-bundle-for-test"),
            ..ZincConfig::default()
        });

        let error = runtime
            .play_with_unsafe_opt_in(&[], None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("zinc-play is disabled by default"));
        assert!(!error.contains("Wine executable not found"));
        assert!(!error.contains("ZiNc.exe not found"));
    }

    #[test]
    fn zinc_check_reports_default_deny_gate() {
        let report = ZincRuntime::new(ZincConfig::default()).check();

        assert!(report.contains("\"zinc_play_default_denied\":true"));
        assert!(report.contains("\"unsafe_opt_in_env\""));
        assert!(report.contains(UNSAFE_ZINC_WINE_ENV));
    }

    #[test]
    fn archive_entry_validation_rejects_path_escape() {
        assert!(archive_entry_is_safe("bundle/roms/game.zip"));
        assert!(!archive_entry_is_safe("../outside"));
        assert!(!archive_entry_is_safe("bundle/../../outside"));
        assert!(!archive_entry_is_safe("/absolute/path"));
        assert!(!archive_entry_is_safe("\\windows\\absolute"));
    }
}
