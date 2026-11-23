//! Where RunIt keeps things.
use serde::{de::DeserializeOwned, Serialize};
use std::path::{Path, PathBuf};

/// `RUNIT_DATA_DIR`, else the usual place for the platform.
pub fn data_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("RUNIT_DATA_DIR") {
        return d.into();
    }
    let home = || PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("RunIt")
    } else if cfg!(target_os = "macos") {
        home().join("Library/Application Support/RunIt")
    } else {
        home().join(".local/share/runit")
    }
}

pub fn load<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Write beside, then rename, so a crash never leaves half a file.
pub fn save<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    std::fs::write(&tmp, serde_json::to_vec(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
