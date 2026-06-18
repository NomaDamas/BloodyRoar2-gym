use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::backend::BackendError;

#[derive(Clone, Debug)]
pub struct NativeRomSet {
    pub path: PathBuf,
    pub entries: Vec<String>,
}

impl NativeRomSet {
    pub fn inspect(path: impl Into<PathBuf>) -> Result<Self, BackendError> {
        let path = path.into();
        let output = Command::new("zipinfo")
            .arg("-1")
            .arg(&path)
            .output()
            .map_err(|error| BackendError::new(format!("failed to run zipinfo: {error}")))?;

        if !output.status.success() {
            return Err(BackendError::new(format!(
                "failed to inspect {}",
                path.display()
            )));
        }

        let entries = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect();

        Ok(Self { path, entries })
    }

    pub fn load_boot_rom(&self) -> Result<Vec<u8>, BackendError> {
        let preferred = ["coh-1002e.353", "m27c402cz-54.ic353"];
        for candidate in preferred {
            if self.entries.iter().any(|entry| entry == candidate) {
                return unzip_entry(&self.path, candidate);
            }
        }

        Err(BackendError::new(
            "no supported boot ROM entry found in ROM set",
        ))
    }

    pub fn json(&self) -> String {
        let entries = self
            .entries
            .iter()
            .map(|entry| format!("\"{}\"", entry.replace('"', "'")))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"path\":\"{}\",\"entry_count\":{},\"entries\":[{}]}}",
            self.path.display(),
            self.entries.len(),
            entries
        )
    }
}

fn unzip_entry(path: &Path, entry: &str) -> Result<Vec<u8>, BackendError> {
    let temp_path = std::env::temp_dir().join(format!("bloodyroar2-{entry}"));
    let status = Command::new("unzip")
        .arg("-p")
        .arg(path)
        .arg(entry)
        .output()
        .map_err(|error| BackendError::new(format!("failed to run unzip: {error}")))?;

    if !status.status.success() {
        return Err(BackendError::new(format!(
            "failed to extract {entry} from {}",
            path.display()
        )));
    }

    fs::write(&temp_path, &status.stdout)
        .map_err(|error| BackendError::new(format!("failed to write temp ROM: {error}")))?;
    let bytes = fs::read(&temp_path)
        .map_err(|error| BackendError::new(format!("failed to read temp ROM: {error}")))?;
    let _ = fs::remove_file(temp_path);
    Ok(bytes)
}
