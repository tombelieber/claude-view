use std::sync::Arc;

use tracing::info;

use super::client::LlmClient;
use super::config::LocalLlmConfig;
use super::lifecycle;
use super::status::{LlmStatus, StatusSnapshot};

/// Local LLM integration service. Arc-friendly — clone cheaply, share everywhere.
pub struct LocalLlmService {
    pub config: Arc<LocalLlmConfig>,
    pub status: Arc<LlmStatus>,
}

impl LocalLlmService {
    pub fn new(config: Arc<LocalLlmConfig>, status: Arc<LlmStatus>) -> Self {
        Self { config, status }
    }

    /// Spawn the background lifecycle task. Call once at startup.
    pub fn start_lifecycle(&self) {
        let status = self.status.clone();
        let config = self.config.clone();
        tokio::spawn(lifecycle::run_lifecycle(status, config));
        info!("LLM lifecycle task spawned");
    }

    /// Create a classify client wired to this service's ready flag.
    pub fn client(&self, debug_tx: Option<tokio::sync::mpsc::Sender<String>>) -> LlmClient {
        // URL is read dynamically from status at request time.
        // For now, use the current URL or default oMLX port.
        let base_url = self
            .status
            .snapshot()
            .url
            .unwrap_or_else(|| "http://localhost:10710".to_string());

        let mut c = LlmClient::new(base_url, self.status.active_model_ref())
            .with_ready_flag(self.status.ready.clone());
        if let Some(tx) = debug_tx {
            c = c.with_debug_tx(tx);
        }
        c
    }

    /// Enable auto-detect. Lifecycle will start probing.
    pub fn enable(&self) -> Result<(), String> {
        self.config
            .set_enabled(true)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        info!("local LLM enabled");
        Ok(())
    }

    /// Disable. Lifecycle will stop probing, clear connection.
    pub fn disable(&self) -> Result<(), String> {
        self.config
            .set_enabled(false)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.status.clear_connection();
        info!("local LLM disabled");
        Ok(())
    }

    /// Set a custom URL. Lifecycle will probe only this URL.
    pub fn connect(&self, url: String) -> Result<(), String> {
        self.config
            .set_url(Some(url.clone()))
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.config
            .set_enabled(true)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        info!(url = %url, "custom URL set");
        Ok(())
    }

    /// Clear custom URL and disconnect.
    pub fn disconnect(&self) -> Result<(), String> {
        self.config
            .set_url(None)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.status.clear_connection();
        info!("disconnected");
        Ok(())
    }

    /// Set the active model. Validates it exists in the provider's model list,
    /// persists to config, and updates live status immediately.
    pub fn set_model(&self, model_id: &str) -> Result<(), String> {
        let snap = self.status.snapshot();
        if !snap.models.contains(&model_id.to_string()) {
            return Err(format!("model '{model_id}' not available on server"));
        }
        self.config
            .set_active_model(Some(model_id.to_string()))
            .map_err(|e| format!("config persist failed: {e}"))?;
        *self.status.active_model_ref().write().unwrap() = Some(model_id.to_string());
        tracing::info!(model = %model_id, "active model changed");
        Ok(())
    }

    /// Graceful shutdown. No-op since we connect to external servers, not manage processes.
    pub async fn shutdown_managed(&self) {
        self.status.clear_connection();
        tracing::info!("local LLM shutdown complete");
    }

    /// Snapshot for the status route.
    pub fn status_snapshot(&self) -> StatusSnapshot {
        let mut snap = self.status.snapshot();
        // Merge config fields into snapshot
        snap.enabled = self.config.enabled();
        snap.classify_mode = self.config.classify_mode();
        snap
    }
}
