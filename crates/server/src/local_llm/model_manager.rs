use std::path::PathBuf;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::download::{self, DownloadProgress};
use super::inventory;
use super::registry::{self, REGISTRY};

#[derive(Debug)]
pub struct ModelManager {
    models_dir: PathBuf,
    inventory_path: PathBuf,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelManager {
    pub fn new() -> Self {
        let base = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".claude-view")
            .join("local-llm");
        Self {
            models_dir: base.join("models"),
            inventory_path: base.join("inventory.json"),
        }
    }

    /// Check if a model is downloaded and valid per the inventory.
    pub fn is_downloaded(&self, model_id: &str) -> bool {
        let known_ids: Vec<&str> = REGISTRY.iter().map(|m| m.id).collect();
        let inv = inventory::load_validated(&self.inventory_path, &self.models_dir, &known_ids);
        inv.downloaded.contains_key(model_id)
    }

    /// Path to the models directory (used by process spawner).
    pub fn models_dir(&self) -> &std::path::Path {
        &self.models_dir
    }

    /// Return the directory path for a given model.
    pub fn model_dir(&self, model_id: &str) -> PathBuf {
        self.models_dir.join(model_id)
    }

    /// Ensure a model is present. Returns:
    /// - `Ok(None)` — model already downloaded, no action needed.
    /// - `Ok(Some((rx, cancel)))` — download started with progress stream + cancel handle.
    /// - `Err(msg)` — setup failure.
    pub async fn ensure_model(
        &self,
        model_id: &str,
    ) -> Result<Option<(mpsc::Receiver<DownloadProgress>, CancellationToken)>, String> {
        if self.is_downloaded(model_id) {
            info!(model_id, "model already present");
            return Ok(None);
        }

        let entry =
            registry::find_model(model_id).ok_or_else(|| format!("unknown model: {model_id}"))?;

        let (tx, rx) = mpsc::channel::<DownloadProgress>(32);
        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();
        let model_dir = self.model_dir(model_id);
        let hf_repo = entry.hf_repo.to_string();
        let inventory_path = self.inventory_path.clone();
        let entry_id = model_id.to_string();
        let entry_size = entry.size_bytes;

        tokio::spawn(async move {
            match download::download_repo(&hf_repo, &model_dir, tx.clone(), cancel_clone).await {
                Ok(file_count) => {
                    let mut inv = inventory::load(&inventory_path);
                    inv.downloaded.insert(
                        entry_id,
                        inventory::InventoryEntry {
                            downloaded_at: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            size_bytes: entry_size,
                            file_count,
                            verified: true,
                        },
                    );
                    if let Err(e) = inventory::save(&inventory_path, &inv) {
                        warn!(%e, "failed to save inventory after download");
                    }
                    info!(file_count, "model download complete");
                }
                Err(e) if e == "cancelled" => {
                    info!("model download cancelled by user");
                }
                Err(e) => {
                    warn!(%e, "model download failed");
                    let _ = tx
                        .send(DownloadProgress {
                            bytes_downloaded: 0,
                            total_bytes: None,
                            percent: None,
                            file_name: None,
                            files_done: 0,
                            files_total: 0,
                            speed_bytes_per_sec: None,
                            eta_secs: None,
                            done: true,
                            error: Some(e),
                        })
                        .await;
                }
            }
        });

        Ok(Some((rx, cancel)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn manager_at(dir: &std::path::Path) -> ModelManager {
        ModelManager {
            models_dir: dir.join("models"),
            inventory_path: dir.join("inventory.json"),
        }
    }

    #[test]
    fn is_downloaded_false_when_empty() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        assert!(!mgr.is_downloaded("qwen3.5-4b-mlx-4bit"));
    }

    #[test]
    fn is_downloaded_true_after_inventory_populated() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());

        let model_dir = mgr.model_dir("qwen3.5-4b-mlx-4bit");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"{}").unwrap();

        let mut inv = inventory::Inventory::default();
        inv.downloaded.insert(
            "qwen3.5-4b-mlx-4bit".into(),
            inventory::InventoryEntry {
                downloaded_at: 0,
                size_bytes: 100,
                file_count: 1,
                verified: true,
            },
        );
        inventory::save(&mgr.inventory_path, &inv).unwrap();

        assert!(mgr.is_downloaded("qwen3.5-4b-mlx-4bit"));
    }

    #[test]
    fn model_dir_returns_expected_path() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let path = mgr.model_dir("qwen3.5-4b-mlx-4bit");
        assert!(path.ends_with("models/qwen3.5-4b-mlx-4bit"));
    }

    #[tokio::test]
    async fn ensure_model_returns_none_when_downloaded() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());

        let model_dir = mgr.model_dir("qwen3.5-4b-mlx-4bit");
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("config.json"), b"{}").unwrap();
        let mut inv = inventory::Inventory::default();
        inv.downloaded.insert(
            "qwen3.5-4b-mlx-4bit".into(),
            inventory::InventoryEntry {
                downloaded_at: 0,
                size_bytes: 100,
                file_count: 1,
                verified: true,
            },
        );
        inventory::save(&mgr.inventory_path, &inv).unwrap();

        let result = mgr.ensure_model("qwen3.5-4b-mlx-4bit").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn ensure_model_rejects_unknown_model() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let result = mgr.ensure_model("nonexistent-model").await;
        assert!(result.is_err());
    }
}
