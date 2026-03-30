use std::sync::atomic::Ordering;
use std::sync::Arc;

use tracing::info;

use super::client::LlmClient;
use super::config::LocalLlmConfig;
use super::lifecycle::{self, ProcessMode};
use super::model_manager::ModelManager;
use super::status::{LlmStatus, StatusSnapshot};

/// Standalone local LLM service. Arc-friendly — clone cheaply, share everywhere.
/// Consumers call `service.client()` for an LlmClient handle.
/// Routes call `service.enable()` / `service.disable()` / `service.status_snapshot()`.
pub struct LocalLlmService {
    pub config: Arc<LocalLlmConfig>,
    pub status: Arc<LlmStatus>,
    model_manager: ModelManager,
}

impl LocalLlmService {
    pub fn new(config: Arc<LocalLlmConfig>, status: Arc<LlmStatus>) -> Self {
        Self {
            config,
            status,
            model_manager: ModelManager::new(),
        }
    }

    /// Spawn the background lifecycle task. Call once at startup.
    pub fn start_lifecycle(&self) {
        let status = self.status.clone();
        let config = self.config.clone();
        let mode = ProcessMode::Discover { port: status.port };
        tokio::spawn(lifecycle::run_lifecycle(status, config, mode));
        info!("LLM lifecycle task spawned");
    }

    /// Create a classify client wired to this service's ready flag.
    pub fn client(
        &self,
        model: String,
        debug_tx: Option<tokio::sync::mpsc::Sender<String>>,
    ) -> LlmClient {
        let mut c = LlmClient::new(format!("http://localhost:{}", self.status.port), model)
            .with_ready_flag(self.status.ready.clone());
        if let Some(tx) = debug_tx {
            c = c.with_debug_tx(tx);
        }
        c
    }

    /// Enable on-device AI. Persists to disk, ensures model, lifecycle will pick up.
    pub async fn enable(
        &self,
    ) -> Result<Option<tokio::sync::mpsc::Receiver<super::download::DownloadProgress>>, String>
    {
        self.config
            .set_enabled(true)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        info!("on-device AI enabled");
        self.model_manager
            .ensure_model(super::registry::default_model().id)
            .await
    }

    /// Disable on-device AI. Dual-path: immediate ready=false + persist to disk.
    pub fn disable(&self) -> Result<(), String> {
        self.config
            .set_enabled(false)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.status.ready.store(false, Ordering::Release);
        self.status.set_pid(None);
        info!("on-device AI disabled");
        Ok(())
    }

    /// Snapshot for the status route.
    pub fn status_snapshot(&self) -> ServiceStatus {
        ServiceStatus {
            enabled: self.config.enabled(),
            llm: self.status.snapshot(),
            model_exists: self
                .model_manager
                .is_downloaded(super::registry::default_model().id),
            model_size_bytes: Some(super::registry::default_model().size_bytes),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceStatus {
    pub enabled: bool,
    pub llm: StatusSnapshot,
    pub model_exists: bool,
    pub model_size_bytes: Option<u64>,
}
