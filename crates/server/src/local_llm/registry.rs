use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ModelEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub hf_repo: &'static str,
    pub size_bytes: u64,
    pub min_ram_gb: u8,
    pub default: bool,
    pub model_id_substring: &'static str,
}

pub const REGISTRY: &[ModelEntry] = &[
    ModelEntry {
        id: "qwen3.5-4b-mlx-4bit",
        name: "Qwen 3.5 4B",
        hf_repo: "mlx-community/Qwen3.5-4B-MLX-4bit",
        size_bytes: 2_500_000_000,
        min_ram_gb: 4,
        default: true,
        model_id_substring: "Qwen3.5-4B",
    },
    ModelEntry {
        id: "qwen3-8b-mlx-4bit",
        name: "Qwen 3 8B",
        hf_repo: "mlx-community/Qwen3-8B-MLX-4bit",
        size_bytes: 5_000_000_000,
        min_ram_gb: 8,
        default: false,
        model_id_substring: "Qwen3-8B",
    },
];

pub fn find_model(id: &str) -> Option<&'static ModelEntry> {
    REGISTRY.iter().find(|m| m.id == id)
}

pub fn default_model() -> &'static ModelEntry {
    REGISTRY
        .iter()
        .find(|m| m.default)
        .expect("registry must have a default model")
}

/// Returns total system RAM in GB, or None if detection fails.
/// Callers treat None as "allow all models" (fail-open for power users).
pub fn total_ram_gb() -> Option<u64> {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let bytes = sys.total_memory();
    if bytes == 0 {
        return None;
    }
    Some(bytes / (1024 * 1024 * 1024))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_existing_model() {
        let m = find_model("qwen3.5-4b-mlx-4bit").unwrap();
        assert_eq!(m.name, "Qwen 3.5 4B");
        assert!(m.default);
    }

    #[test]
    fn find_unknown_returns_none() {
        assert!(find_model("nonexistent").is_none());
    }

    #[test]
    fn default_model_exists_and_is_unique() {
        let m = default_model();
        assert!(m.default);
        let count = REGISTRY.iter().filter(|m| m.default).count();
        assert_eq!(count, 1, "exactly one model must be marked as default");
    }

    #[test]
    fn all_entries_have_nonempty_fields() {
        for entry in REGISTRY {
            assert!(!entry.id.is_empty());
            assert!(!entry.name.is_empty());
            assert!(!entry.hf_repo.is_empty());
            assert!(entry.size_bytes > 0);
            assert!(entry.min_ram_gb > 0);
            assert!(!entry.model_id_substring.is_empty());
        }
    }

    #[test]
    fn ram_detection_returns_some_on_real_machine() {
        let gb = total_ram_gb();
        assert!(gb.is_some());
        assert!(gb.unwrap() > 0);
    }
}
