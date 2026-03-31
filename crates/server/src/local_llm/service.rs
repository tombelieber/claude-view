use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use tracing::info;

use super::client::LlmClient;
use super::config::LocalLlmConfig;
use super::download::DownloadProgress;
use super::lifecycle::{self, ProcessMode};
use super::model_manager::ModelManager;
use super::omlx_binary;
use super::process::ManagedProcess;
use super::registry::{self, REGISTRY};
use super::status::{LlmStatus, StatusSnapshot};

/// Standalone local LLM service. Arc-friendly — clone cheaply, share everywhere.
pub struct LocalLlmService {
    pub config: Arc<LocalLlmConfig>,
    pub status: Arc<LlmStatus>,
    model_manager: ModelManager,
    /// Active download cancellation handle (if a download is in progress).
    active_download_cancel: Mutex<Option<CancellationToken>>,
    /// Owned oMLX child process. None if using Discover mode (external).
    managed_process: Mutex<Option<ManagedProcess>>,
}

impl LocalLlmService {
    pub fn new(config: Arc<LocalLlmConfig>, status: Arc<LlmStatus>) -> Self {
        Self {
            config,
            status,
            model_manager: ModelManager::new(),
            active_download_cancel: Mutex::new(None),
            managed_process: Mutex::new(None),
        }
    }

    /// Spawn the background lifecycle task. Call once at startup.
    /// Detects whether an external oMLX is already running (Discover)
    /// or we should manage it ourselves (Managed).
    pub fn start_lifecycle(&self) {
        let status = self.status.clone();
        let config = self.config.clone();

        if omlx_binary::is_port_in_use(status.port) {
            // External oMLX detected — use Discover mode
            info!(
                port = status.port,
                "external oMLX detected, using Discover mode"
            );
            let mode = ProcessMode::Discover { port: status.port };
            tokio::spawn(lifecycle::run_lifecycle(status, config, mode));
        } else {
            // No external oMLX — use Discover mode for now
            // Managed process spawned in enable()
            info!(
                port = status.port,
                "no external oMLX, Managed mode available"
            );
            let mode = ProcessMode::Discover { port: status.port };
            tokio::spawn(lifecycle::run_lifecycle(status, config, mode));
        }

        info!("LLM lifecycle task spawned");
    }

    /// Create a classify client wired to this service's ready flag.
    /// Model ID is read dynamically from LlmStatus (set by lifecycle).
    pub fn client(&self, debug_tx: Option<tokio::sync::mpsc::Sender<String>>) -> LlmClient {
        let mut c = LlmClient::new(
            format!("http://localhost:{}", self.status.port),
            self.status.discovered_model_ref(),
        )
        .with_ready_flag(self.status.ready.clone());
        if let Some(tx) = debug_tx {
            c = c.with_debug_tx(tx);
        }
        c
    }

    /// Enable on-device AI. Spawns oMLX if not already running.
    pub async fn enable(
        &self,
    ) -> Result<Option<tokio::sync::mpsc::Receiver<DownloadProgress>>, String> {
        self.config
            .set_enabled(true)
            .map_err(|e| format!("failed to persist config: {e}"))?;

        let model = self.resolve_active_model();
        info!(model_id = model.id, "on-device AI enabled");

        // 1. Download model if needed
        let rx = self.start_download(model.id).await?;

        // 2. If download is in progress, return the progress stream.
        // Spawn will happen after download completes (lifecycle detects model).
        // If no download needed, try to spawn now.
        if rx.is_some() {
            return Ok(rx);
        }

        // 3. If oMLX already running on port, nothing to do (Discover mode)
        if omlx_binary::is_port_in_use(self.status.port) {
            info!("oMLX already running on port, using Discover mode");
            return Ok(None);
        }

        // 4. Spawn managed process
        self.spawn_omlx().await?;

        Ok(None)
    }

    /// Spawn oMLX as a managed child process.
    async fn spawn_omlx(&self) -> Result<(), String> {
        let binary =
            omlx_binary::detect().ok_or("omlx not found. Install with: pip install omlx")?;

        if !omlx_binary::verify(&binary) {
            return Err("omlx binary found but failed verification".into());
        }

        let process = ManagedProcess::spawn(
            &binary.path,
            self.model_manager.models_dir(),
            self.status.port,
        )
        .await?;

        *self.managed_process.lock().unwrap() = Some(process);
        Ok(())
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

        self.start_download(model_id).await
    }

    /// Cancel any in-progress download.
    pub fn cancel_download(&self) {
        if let Some(cancel) = self.active_download_cancel.lock().unwrap().take() {
            cancel.cancel();
            info!("download cancelled by user");
        }
    }

    async fn start_download(
        &self,
        model_id: &str,
    ) -> Result<Option<tokio::sync::mpsc::Receiver<DownloadProgress>>, String> {
        match self.model_manager.ensure_model(model_id).await? {
            Some((rx, cancel)) => {
                *self.active_download_cancel.lock().unwrap() = Some(cancel);
                Ok(Some(rx))
            }
            None => Ok(None),
        }
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

    /// Disable on-device AI. Kills managed process if we own it.
    pub fn disable(&self) -> Result<(), String> {
        self.config
            .set_enabled(false)
            .map_err(|e| format!("failed to persist config: {e}"))?;
        self.status.ready.store(false, Ordering::Release);
        self.status.set_pid(None);
        self.status.set_discovered_model_id(None);
        self.status
            .set_server_state(super::status::ServerState::Unknown);

        // Kill managed process if we own it
        if let Some(mut process) = self.managed_process.lock().unwrap().take() {
            tokio::spawn(async move { process.shutdown().await });
        }

        info!("on-device AI disabled");
        Ok(())
    }

    /// Snapshot for the status route.
    pub fn status_snapshot(&self) -> ServiceStatus {
        let active = self.resolve_active_model();
        let mode = if !self.config.enabled() {
            "none"
        } else if self.managed_process.lock().unwrap().is_some() {
            "managed"
        } else {
            "external"
        };
        ServiceStatus {
            enabled: self.config.enabled(),
            llm: self.status.snapshot(),
            model_exists: self.model_manager.is_downloaded(active.id),
            model_size_bytes: Some(active.size_bytes),
            active_model_id: active.id.to_string(),
            mode,
        }
    }

    /// Shutdown managed process. Called during app exit.
    pub async fn shutdown_managed(&self) {
        if let Some(mut process) = self.managed_process.lock().unwrap().take() {
            process.shutdown().await;
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
    pub mode: &'static str,
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
