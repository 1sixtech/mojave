use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};

pub fn resolve_data_dir(datadir: &str) -> String {
    let path = match std::env::home_dir() {
        Some(home) => home.join(datadir),
        None => PathBuf::from(".").join(datadir),
    };

    if !path.exists() {
        std::fs::create_dir_all(&path).expect("Failed to create the data directory.");
    }

    path.to_str()
        .expect("Invalid UTF-8 in data directory")
        .to_owned()
}

pub fn resolve_path<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    if path.as_ref().is_absolute() {
        let absolute = path.as_ref().to_path_buf();
        if let Some(parent) = absolute.parent().filter(|p| !p.exists()) {
            fs::create_dir_all(parent)?;
        }
        return Ok(absolute);
    }

    let path_buf = match std::env::home_dir() {
        Some(home) => home.join(&path),
        None => PathBuf::from(".").join(&path),
    };

    if let Some(parent) = path_buf.parent().filter(|p| !p.exists()) {
        fs::create_dir_all(parent)?;
    }
    Ok(path_buf)
}

pub fn default_settings_path(service: &str, datadir: &str) -> PathBuf {
    let base = PathBuf::from(resolve_data_dir(datadir));
    base.join(format!("{}.settings.json", service))
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let file = File::open(path).ok()?;
    serde_json::from_reader::<_, T>(file).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    let json =
        serde_json::to_vec_pretty(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(path, json)
}
