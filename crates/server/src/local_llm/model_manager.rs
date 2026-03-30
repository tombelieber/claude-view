use std::path::PathBuf;

use serde::Serialize;
use tokio::sync::mpsc;
use tracing::{info, warn};

const MODEL_DIR: &str = "models";
const DEFAULT_MODEL_FILENAME: &str = "Qwen3.5-4B-MLX-4bit";

/// Minimum file size (1 MB) to consider a model file valid.
/// Anything smaller is a stub or corrupt download.
const MIN_MODEL_SIZE: u64 = 1_000_000;

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f32>,
    pub done: bool,
}

#[derive(Debug)]
pub struct ModelManager {
    model_dir: PathBuf,
}

impl Default for ModelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelManager {
    /// Create a ModelManager using the app data directory.
    /// `~/.claude-view/local-llm/models` — centralized, survives reinstalls.
    pub fn new() -> Self {
        let model_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".claude-view")
            .join("local-llm")
            .join(MODEL_DIR);
        Self { model_dir }
    }

    /// Check whether the model file exists and is larger than 1 MB.
    pub fn model_exists(&self) -> bool {
        self.model_path()
            .metadata()
            .map(|m| m.len() >= MIN_MODEL_SIZE)
            .unwrap_or(false)
    }

    /// Return the full path to the model file.
    pub fn model_path(&self) -> PathBuf {
        self.model_dir.join(DEFAULT_MODEL_FILENAME)
    }

    /// Return the model file size in bytes, or None if the file doesn't exist.
    pub fn model_size_bytes(&self) -> Option<u64> {
        self.model_path().metadata().ok().map(|m| m.len())
    }

    /// Ensure the model is present. Returns:
    /// - `Ok(None)` if the model already exists (no download needed).
    /// - `Ok(Some(rx))` if a download was started (receiver streams progress).
    /// - `Err(msg)` if something went wrong.
    pub async fn ensure_model(&self) -> Result<Option<mpsc::Receiver<DownloadProgress>>, String> {
        if self.model_exists() {
            info!(path = %self.model_path().display(), "model already present");
            return Ok(None);
        }

        std::fs::create_dir_all(&self.model_dir)
            .map_err(|e| format!("failed to create model dir: {e}"))?;

        let (tx, rx) = mpsc::channel::<DownloadProgress>(32);
        let model_path = self.model_path();

        tokio::spawn(async move {
            if let Err(e) = download_model(&model_path, tx.clone()).await {
                warn!(%e, "model download failed");
                let partial = model_path.with_extension("partial");
                let _ = std::fs::remove_file(&partial);
                let _ = tx
                    .send(DownloadProgress {
                        bytes_downloaded: 0,
                        total_bytes: None,
                        percent: None,
                        done: true,
                    })
                    .await;
            }
        });

        Ok(Some(rx))
    }
}

async fn download_model(
    model_path: &std::path::Path,
    tx: mpsc::Sender<DownloadProgress>,
) -> Result<(), String> {
    // TODO: implement actual HuggingFace CDN download when distribution strategy is chosen.
    info!(path = %model_path.display(), "model download: awaiting distribution strategy");
    let _ = tx
        .send(DownloadProgress {
            bytes_downloaded: 0,
            total_bytes: None,
            percent: None,
            done: true,
        })
        .await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn manager_at(dir: &std::path::Path) -> ModelManager {
        ModelManager {
            model_dir: dir.to_path_buf(),
        }
    }

    #[test]
    fn model_exists_returns_false_when_missing() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        assert!(!mgr.model_exists());
    }

    #[test]
    fn model_exists_returns_false_for_tiny_file() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        std::fs::write(mgr.model_path(), "tiny").unwrap();
        assert!(!mgr.model_exists()); // <1MB = stub
    }

    #[test]
    fn model_exists_returns_true_for_large_file() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let data = vec![0u8; 2_000_000]; // 2MB
        std::fs::write(mgr.model_path(), &data).unwrap();
        assert!(mgr.model_exists());
    }

    #[test]
    fn model_path_uses_default_filename() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        assert!(mgr.model_path().ends_with(DEFAULT_MODEL_FILENAME));
    }

    #[test]
    fn model_size_bytes_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        assert_eq!(mgr.model_size_bytes(), None);
    }

    #[test]
    fn model_size_bytes_returns_actual_size() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let data = vec![0u8; 5_000];
        std::fs::write(mgr.model_path(), &data).unwrap();
        assert_eq!(mgr.model_size_bytes(), Some(5_000));
    }

    #[tokio::test]
    async fn ensure_model_returns_none_when_exists() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let data = vec![0u8; 2_000_000];
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::write(mgr.model_path(), &data).unwrap();
        let result = mgr.ensure_model().await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn ensure_model_returns_rx_when_missing() {
        let dir = tempdir().unwrap();
        let mgr = manager_at(dir.path());
        let result = mgr.ensure_model().await.unwrap();
        assert!(result.is_some()); // download stream started
    }
}
