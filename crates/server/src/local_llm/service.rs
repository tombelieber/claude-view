use std::sync::atomic::Ordering;
use std::sync::Arc;

use tracing::info;

use super::client::LlmClient;
use super::config::LocalLlmConfig;
use super::download::DownloadProgress;
use super::lifecycle::{self, ProcessMode};
use super::model_manager::ModelManager;
use super::registry::{self, REGISTRY};
use super::status::{LlmStatus, StatusSnapshot};

/// Standalone local LLM service. Arc-friendly — clone cheaply, share everywhere.
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

    /// Enable on-device AI. Persists to disk, ensures active model is downloaded.
    pub async fn enable(
        &self,
    ) -> Result<Option<tokio::sync::mpsc::Receiver<DownloadProgress>>, String> {
        self.config
            .set_enabled(true)
            .map_err(|e| format!("failed to persist config: {e}"))?;

        let model = self.resolve_active_model();
        info!(model_id = model.id, "on-device AI enabled");
        self.model_manager.ensure_model(model.id).await
    }

    /// Switch to a different model. Downloads if needed.
    pub async fn switch_model(
        &self,
        model_id: &str,
    ) -> Result<Option<tokio::sync::mpsc::Receiver<DownloadProgress>>, String> {
        let entry =
            registry::find_model(model_id).ok_or_else(|| format!("unknown model: {model_id}"))?;

        // RAM guard: block if machine can't run this model
        if let Some(gb) = registry::total_ram_gb() {
            if gb < entry.min_ram_gb as u64 {
                return Err(format!(
                    "insufficient RAM: {} GB available, {} GB required",
                    gb, entry.min_ram_gb
                ));
            }
        }

        self.config
            .set_active_model(Some(model_id.to_string()))
            .map_err(|e| format!("failed to persist config: {e}"))?;
        info!(model_id, "model switched");

        // Download if needed — lifecycle will pick up the new model on next poll
        self.model_manager.ensure_model(model_id).await
    }

    /// List all models with installed/active/can_run status.
    pub fn models_list(&self) -> Vec<ModelInfo> {
        let active_id = self.resolve_active_model().id;
        let ram = registry::total_ram_gb();

        REGISTRY
            .iter()
            .map(|entry| ModelInfo {
                id: entry.id,
                name: entry.name,
                size_bytes: entry.size_bytes,
                min_ram_gb: entry.min_ram_gb,
                installed: self.model_manager.is_downloaded(entry.id),
                active: entry.id == active_id,
                can_run: ram.map_or(true, |gb| gb >= entry.min_ram_gb as u64),
            })
            .collect()
    }

    /// Disable on-device AI. Dual-path: immediate ready=false + persist to disk.
    pub fn disable(&self) -> Result<(), String> {
        self.config
            .set_enabled(false)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.status.ready.store(false, Ordering::Release);
        self.status.set_pid(None);
        self.status.set_server_state(super::status::ServerState::Unknown);
        info!("on-device AI disabled");
        Ok(())
    }

    /// Snapshot for the status route.
    pub fn status_snapshot(&self) -> ServiceStatus {
        let active = self.resolve_active_model();
        ServiceStatus {
            enabled: self.config.enabled(),
            llm: self.status.snapshot(),
            model_exists: self.model_manager.is_downloaded(active.id),
            model_size_bytes: Some(active.size_bytes),
            active_model_id: active.id.to_string(),
        }
    }

    fn resolve_active_model(&self) -> &'static registry::ModelEntry {
        self.config
            .active_model()
            .and_then(|id| registry::find_model(&id))
            .unwrap_or_else(registry::default_model)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ServiceStatus {
    pub enabled: bool,
    pub llm: StatusSnapshot,
    pub model_exists: bool,
    pub model_size_bytes: Option<u64>,
    pub active_model_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub size_bytes: u64,
    pub min_ram_gb: u8,
    pub installed: bool,
    pub active: bool,
    pub can_run: bool,
}
