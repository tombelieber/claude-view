//! API key generation, validation, and storage.
//!
//! Key format: `cv_live_{32_base62}_{8_hex_crc32}`
//! Storage: SHA-256 hash in `~/.claude-view/api-keys.json`

use chrono::Utc;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

const BASE62_CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeyEntry {
    pub id: String,
    pub hash: String,
    pub prefix: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApiKeyStore {
    pub keys: Vec<ApiKeyEntry>,
}

/// Generate a new API key.
///
/// Returns the raw key (shown once to the user) and the entry to persist in the store.
pub fn generate_key() -> (String, ApiKeyEntry) {
    let mut rng = rand::thread_rng();

    // 32-char base62 random part
    let random_part: String = (0..32)
        .map(|_| {
            let idx = rng.gen_range(0..BASE62_CHARS.len());
            BASE62_CHARS[idx] as char
        })
        .collect();

    // 8-char hex CRC32 checksum of the random part
    let crc = crc32fast::hash(random_part.as_bytes());
    let checksum = format!("{crc:08x}");

    let raw = format!("cv_live_{random_part}_{checksum}");

    // SHA-256 hash of the full raw key for server-side storage
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = hex::encode(hasher.finalize());

    // First 12 chars as a human-readable prefix (e.g. "cv_live_ABCD")
    let prefix = raw[..12].to_string();

    let entry = ApiKeyEntry {
        id: Uuid::new_v4().to_string(),
        hash,
        prefix,
        created_at: Utc::now().to_rfc3339(),
        last_used_at: None,
    };

    (raw, entry)
}

/// Validate a raw API key against the store.
///
/// Returns the key's id if valid, or None if not found.
pub fn validate_key(raw: &str, store: &ApiKeyStore) -> Option<String> {
    if !raw.starts_with("cv_live_") {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let hash = hex::encode(hasher.finalize());

    store
        .keys
        .iter()
        .find(|entry| entry.hash == hash)
        .map(|entry| entry.id.clone())
}

/// Load the API key store from a JSON file. Returns an empty store if the file is missing.
pub fn load_store(path: &PathBuf) -> ApiKeyStore {
    match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => ApiKeyStore::default(),
    }
}

/// Persist the API key store to a JSON file, creating parent directories as needed.
pub fn save_store(store: &ApiKeyStore, path: &PathBuf) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store).map_err(std::io::Error::other)?;
    std::fs::write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn generated_key_has_correct_format() {
        let (raw, _entry) = generate_key();
        assert!(
            raw.starts_with("cv_live_"),
            "key must start with cv_live_, got: {raw}"
        );
        let parts: Vec<&str> = raw.splitn(4, '_').collect();
        assert_eq!(parts.len(), 4, "key must have 4 underscore-separated parts");
        assert_eq!(parts[2].len(), 32, "random part must be 32 chars");
        assert_eq!(parts[3].len(), 8, "crc32 checksum must be 8 hex chars");
        assert!(
            parts[3].chars().all(|c| c.is_ascii_hexdigit()),
            "crc must be hex"
        );
    }

    #[test]
    fn generated_key_entry_has_sha256_hash() {
        let (raw, entry) = generate_key();
        assert!(!entry.hash.is_empty());
        assert_eq!(entry.hash.len(), 64, "SHA-256 hex is 64 chars");
        assert!(entry.hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(entry.prefix, &raw[..12]);
        assert!(!entry.id.is_empty());
        assert!(!entry.created_at.is_empty());
    }

    #[test]
    fn validate_key_accepts_correct_key() {
        let (raw, entry) = generate_key();
        let store = ApiKeyStore {
            keys: vec![entry.clone()],
        };
        let result = validate_key(&raw, &store);
        assert_eq!(result, Some(entry.id));
    }

    #[test]
    fn validate_key_rejects_wrong_key() {
        let (_raw, entry) = generate_key();
        let store = ApiKeyStore { keys: vec![entry] };
        assert_eq!(validate_key("cv_live_wrong_key_00000000", &store), None);
    }

    #[test]
    fn validate_key_rejects_empty_store() {
        let store = ApiKeyStore::default();
        assert_eq!(validate_key("cv_live_something_00000000", &store), None);
    }

    #[test]
    fn validate_key_rejects_malformed_key() {
        let store = ApiKeyStore::default();
        assert_eq!(validate_key("not-a-key", &store), None);
        assert_eq!(validate_key("", &store), None);
    }

    #[test]
    fn crc32_checksum_is_deterministic() {
        let (raw1, _) = generate_key();
        let parts: Vec<&str> = raw1.splitn(4, '_').collect();
        let random_part = parts[2];
        let expected_crc = parts[3];
        let crc = crc32fast::hash(random_part.as_bytes());
        assert_eq!(format!("{crc:08x}"), expected_crc);
    }

    #[test]
    fn store_roundtrip_through_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("api-keys.json");
        let (_, entry) = generate_key();
        let store = ApiKeyStore {
            keys: vec![entry.clone()],
        };
        save_store(&store, &path).unwrap();
        let loaded = load_store(&path);
        assert_eq!(loaded.keys.len(), 1);
        assert_eq!(loaded.keys[0].id, entry.id);
        assert_eq!(loaded.keys[0].hash, entry.hash);
    }

    #[test]
    fn load_store_returns_empty_on_missing_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");
        let store = load_store(&path);
        assert!(store.keys.is_empty());
    }

    #[test]
    fn two_generated_keys_are_unique() {
        let (raw1, _) = generate_key();
        let (raw2, _) = generate_key();
        assert_ne!(raw1, raw2);
    }
}
