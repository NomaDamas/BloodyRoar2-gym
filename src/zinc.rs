use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::backend::BackendError;

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

        let combined = extract_dir.join("BloodRoar2-combined.zip");
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
            "{{\"wine\":\"{}\",\"wine_found\":{},\"bundle_dir\":\"{}\",\"zinc_exe_found\":{},\"renderer\":\"{}\",\"renderer_found\":{},\"renderer_cfg\":\"{}\",\"renderer_cfg_found\":{},\"bldyror2_rom_found\":{},\"note\":\"ZiNc is a Windows binary. On Apple Silicon it requires Rosetta plus a working Wine install.\"}}",
            self.config.wine.display(),
            command_exists(&self.config.wine),
            self.config.bundle_dir.display(),
            exe.is_file(),
            self.config.renderer,
            renderer.is_file(),
            self.config.renderer_cfg,
            renderer_cfg.is_file(),
            rom.is_file()
        )
    }

    pub fn play(&self, extra_args: &[String]) -> Result<(), BackendError> {
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

fn extract_archive(archive: &Path, extract_dir: &Path) -> Result<(), BackendError> {
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
