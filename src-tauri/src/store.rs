//! Persists the F-key → program mapping as JSON in the app config directory.
//! Keyed by F-key label ("F13".."F24") so it stays human-readable and lines up
//! directly with the global-shortcut codes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Entry {
    /// Path to the app/file/folder, or a URL.
    pub path: String,
    /// Friendly label shown in the UI (defaults to the file name).
    pub name: String,
    /// Action kind: "app" (program/file), "folder", or "url". Older config
    /// files without this field default to "app".
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "app".to_string()
}

pub type Mappings = BTreeMap<String, Entry>;

/// Shared, in-memory mapping guarded by a mutex, plus the file it persists to.
pub struct Store {
    path: PathBuf,
    pub mappings: Mutex<Mappings>,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let mappings = read_file(&path).unwrap_or_default();
        Store {
            path,
            mappings: Mutex::new(mappings),
        }
    }

    pub fn snapshot(&self) -> Mappings {
        self.mappings.lock().unwrap().clone()
    }

    pub fn get(&self, key: &str) -> Option<Entry> {
        self.mappings.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: String, entry: Entry) -> Result<(), String> {
        {
            let mut m = self.mappings.lock().unwrap();
            m.insert(key, entry);
        }
        self.persist()
    }

    pub fn remove(&self, key: &str) -> Result<(), String> {
        {
            let mut m = self.mappings.lock().unwrap();
            m.remove(key);
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let m = self.mappings.lock().unwrap();
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&*m).map_err(|e| e.to_string())?;
        std::fs::write(&self.path, json).map_err(|e| e.to_string())
    }
}

fn read_file(path: &Path) -> Option<Mappings> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}
