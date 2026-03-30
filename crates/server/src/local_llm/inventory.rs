// crates/server/src/local_llm/inventory.rs

use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Inventory {
    pub downloaded: HashMap<String, InventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryEntry {
    pub downloaded_at: u64,
    pub size_bytes: u64,
    pub file_count: u32,
    pub verified: bool,
}

/// Load inventory from disk. Returns empty on any error (fail-closed).
pub fn load(path: &Path) -> Inventory {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save inventory atomically (write .tmp then rename).
pub fn save(path: &Path, inv: &Inventory) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(inv).map_err(std::io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)?;
    std::fs::rename(&tmp, path)
}

/// Remove entries whose model directory is missing or empty on disk.
pub fn validate(inv: &mut Inventory, models_dir: &Path) {
    inv.downloaded.retain(|id, _| {
        let dir = models_dir.join(id);
        dir.is_dir()
            && std::fs::read_dir(&dir)
                .map(|mut d| d.next().is_some())
                .unwrap_or(false)
    });
}

/// Rebuild inventory by scanning models/ directory against a list of known model IDs.
pub fn rebuild_from_disk(models_dir: &Path, known_ids: &[&str]) -> Inventory {
    let mut inv = Inventory::default();
    let entries = match std::fs::read_dir(models_dir) {
        Ok(e) => e,
        Err(_) => return inv,
    };
    for entry in entries.flatten() {
        let dir_name = entry.file_name().to_string_lossy().to_string();
        if !known_ids.contains(&dir_name.as_str()) {
            continue;
        }
        let Ok(files) = std::fs::read_dir(entry.path()) else {
            continue;
        };
        let mut file_count = 0u32;
        let mut total_size = 0u64;
        for f in files.flatten() {
            let Ok(meta) = f.metadata() else { continue };
            if meta.is_file() && !f.file_name().to_string_lossy().ends_with(".partial") {
                file_count += 1;
                total_size += meta.len();
            }
        }
        if file_count > 0 {
            inv.downloaded.insert(
                dir_name,
                InventoryEntry {
                    downloaded_at: now_epoch(),
                    size_bytes: total_size,
                    file_count,
                    verified: false,
                },
            );
        }
    }
    inv
}

/// Load + validate, falling back to rebuild if inventory.json is missing.
pub fn load_validated(inventory_path: &Path, models_dir: &Path, known_ids: &[&str]) -> Inventory {
    if inventory_path.exists() {
        let mut inv = load(inventory_path);
        validate(&mut inv, models_dir);
        inv
    } else {
        rebuild_from_disk(models_dir, known_ids)
    }
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn load_returns_empty_when_missing() {
        let dir = tempdir().unwrap();
        let inv = load(&dir.path().join("nope.json"));
        assert!(inv.downloaded.is_empty());
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("inventory.json");

        let mut inv = Inventory::default();
        inv.downloaded.insert(
            "test-model".into(),
            InventoryEntry {
                downloaded_at: 1000,
                size_bytes: 500,
                file_count: 3,
                verified: true,
            },
        );
        save(&path, &inv).unwrap();

        let loaded = load(&path);
        assert_eq!(loaded.downloaded.len(), 1);
        let entry = &loaded.downloaded["test-model"];
        assert_eq!(entry.file_count, 3);
        assert_eq!(entry.size_bytes, 500);
        assert!(entry.verified);
    }

    #[test]
    fn validate_removes_missing_dirs() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();

        let mut inv = Inventory::default();
        inv.downloaded.insert(
            "exists".into(),
            InventoryEntry {
                downloaded_at: 0,
                size_bytes: 100,
                file_count: 1,
                verified: true,
            },
        );
        inv.downloaded.insert(
            "gone".into(),
            InventoryEntry {
                downloaded_at: 0,
                size_bytes: 100,
                file_count: 1,
                verified: true,
            },
        );

        // Create only the "exists" model dir with a file
        let model_dir = models_dir.join("exists");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("weight.bin"), b"data").unwrap();

        validate(&mut inv, &models_dir);
        assert_eq!(inv.downloaded.len(), 1);
        assert!(inv.downloaded.contains_key("exists"));
    }

    #[test]
    fn rebuild_from_disk_finds_known_models() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");

        // Create a known model directory with files
        let model_dir = models_dir.join("test-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"{}").unwrap();
        std::fs::write(model_dir.join("weights.bin"), b"data").unwrap();
        // .partial files should be excluded
        std::fs::write(model_dir.join("other.partial"), b"incomplete").unwrap();

        // Create an unknown directory (should be ignored)
        let unknown_dir = models_dir.join("random-stuff");
        std::fs::create_dir_all(&unknown_dir).unwrap();
        std::fs::write(unknown_dir.join("file.txt"), b"x").unwrap();

        let inv = rebuild_from_disk(&models_dir, &["test-model"]);
        assert_eq!(inv.downloaded.len(), 1);
        let entry = &inv.downloaded["test-model"];
        assert_eq!(entry.file_count, 2); // config.json + weights.bin (not .partial)
    }

    #[test]
    fn load_validated_falls_back_to_rebuild() {
        let dir = tempdir().unwrap();
        let models_dir = dir.path().join("models");
        let inv_path = dir.path().join("inventory.json");

        // No inventory.json, but model dir exists
        let model_dir = models_dir.join("my-model");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("w.bin"), b"data").unwrap();

        let inv = load_validated(&inv_path, &models_dir, &["my-model"]);
        assert_eq!(inv.downloaded.len(), 1);
        assert!(inv.downloaded.contains_key("my-model"));
    }
}
