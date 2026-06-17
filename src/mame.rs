use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use crate::backend::BackendError;

const REQUIRED_ROM_ARCHIVES: [&str; 4] = ["bldyror2.zip", "cpzn2.zip", "cpzn1.zip", "firmware.zip"];

#[derive(Clone, Debug)]
pub struct MameConfig {
    pub executable: PathBuf,
    pub rom_dir: PathBuf,
    pub game: String,
}

impl Default for MameConfig {
    fn default() -> Self {
        Self {
            executable: PathBuf::from("mame"),
            rom_dir: PathBuf::from("assets/roms"),
            game: "bldyror2".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct MameRuntime {
    config: MameConfig,
}

impl MameRuntime {
    pub fn new(config: MameConfig) -> Self {
        Self { config }
    }

    pub fn prepare_assets(&self, archive: &Path) -> Result<(), BackendError> {
        fs::create_dir_all(&self.config.rom_dir).map_err(|error| {
            BackendError::new(format!(
                "failed to create {}: {error}",
                self.config.rom_dir.display()
            ))
        })?;

        let mut combined_archive = None;

        if self.extract_required_roms(archive).is_err() {
            let combined = self.combined_archive_path(archive)?;
            self.combine_split_zip(archive, &combined)?;
            self.extract_required_roms(&combined)?;
            combined_archive = Some(combined);
        }

        if let Some(combined) = combined_archive {
            eprintln!("combined split archive at {}", combined.display());
        }

        Ok(())
    }

    pub fn combine_split_zip(&self, archive: &Path, combined: &Path) -> Result<(), BackendError> {
        let status = Command::new("zip")
            .arg("-s")
            .arg("0")
            .arg(archive)
            .arg("--out")
            .arg(combined)
            .stdin(Stdio::null())
            .status()
            .map_err(|error| BackendError::new(format!("failed to run zip: {error}")))?;

        status_ok(
            status,
            format!(
                "failed to combine split archive {} into {}",
                archive.display(),
                combined.display()
            ),
        )
    }

    fn extract_required_roms(&self, archive: &Path) -> Result<(), BackendError> {
        for rom_archive in REQUIRED_ROM_ARCHIVES {
            let entry = format!("BloodRoar2/roms/{rom_archive}");
            let status = Command::new("unzip")
                .arg("-jo")
                .arg(archive)
                .arg(&entry)
                .arg("-d")
                .arg(&self.config.rom_dir)
                .stdin(Stdio::null())
                .status()
                .map_err(|error| BackendError::new(format!("failed to run unzip: {error}")))?;

            if !status.success() {
                return Err(BackendError::new(format!(
                    "failed to extract {entry} from {}",
                    archive.display()
                )));
            }
        }

        Ok(())
    }

    fn combined_archive_path(&self, archive: &Path) -> Result<PathBuf, BackendError> {
        let parent = self.config.rom_dir.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|error| {
            BackendError::new(format!("failed to create {}: {error}", parent.display()))
        })?;

        let stem = archive
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("split-archive")
            .replace(' ', "-");
        Ok(parent.join(format!("{stem}-combined.zip")))
    }

    pub fn check(&self) -> Result<String, BackendError> {
        self.ensure_ready()?;
        let output = Command::new(&self.config.executable)
            .arg("-rompath")
            .arg(&self.config.rom_dir)
            .arg("-verifyroms")
            .arg(&self.config.game)
            .output()
            .map_err(|error| {
                BackendError::new(format!(
                    "failed to run {}: {error}",
                    self.config.executable.display()
                ))
            })?;

        let mut report = String::new();
        report.push_str(&String::from_utf8_lossy(&output.stdout));
        report.push_str(&String::from_utf8_lossy(&output.stderr));

        if !output.status.success() {
            return Err(BackendError::new(format!(
                "MAME ROM verification failed:\n{}",
                report.trim()
            )));
        }

        Ok(report)
    }

    pub fn play(&self, extra_args: &[String]) -> Result<(), BackendError> {
        self.ensure_ready()?;
        let status = Command::new(&self.config.executable)
            .arg("-rompath")
            .arg(&self.config.rom_dir)
            .arg(&self.config.game)
            .arg("-window")
            .arg("-skip_gameinfo")
            .args(extra_args)
            .status()
            .map_err(|error| {
                BackendError::new(format!(
                    "failed to launch {}: {error}",
                    self.config.executable.display()
                ))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(BackendError::new(format!("MAME exited with {status}")))
        }
    }

    fn ensure_ready(&self) -> Result<(), BackendError> {
        if !command_exists(&self.config.executable) {
            return Err(BackendError::new(format!(
                "MAME executable not found: {}",
                self.config.executable.display()
            )));
        }

        if !self.config.rom_dir.is_dir() {
            return Err(BackendError::new(format!(
                "ROM directory not found: {}",
                self.config.rom_dir.display()
            )));
        }

        let game_rom = self
            .config
            .rom_dir
            .join(format!("{}.zip", self.config.game));
        if !game_rom.is_file() {
            return Err(BackendError::new(format!(
                "game ROM archive not found: {}",
                game_rom.display()
            )));
        }

        Ok(())
    }
}

fn status_ok(status: ExitStatus, message: String) -> Result<(), BackendError> {
    if status.success() {
        Ok(())
    } else {
        Err(BackendError::new(message))
    }
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
