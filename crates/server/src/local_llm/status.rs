use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use claude_view_core::phase::ClassifyMode;
use serde::Serialize;

use super::provider::Provider;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Unknown = 0,
    Scanning = 1,
    Connected = 2,
    Disconnected = 3,
}

impl From<u8> for ServerState {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Scanning,
            2 => Self::Connected,
            3 => Self::Disconnected,
            _ => Self::Unknown,
        }
    }
}

/// Lock-free shared status for the local LLM integration.
#[derive(Debug)]
pub struct LlmStatus {
    pub ready: Arc<AtomicBool>,
    state: Arc<AtomicU8>,
    provider: Arc<RwLock<Option<Provider>>>,
    url: Arc<RwLock<Option<String>>>,
    models: Arc<RwLock<Vec<String>>>,
    active_model: Arc<RwLock<Option<String>>>,
    omlx_installed: Arc<AtomicBool>,
    omlx_running: Arc<AtomicBool>,
}

impl Default for LlmStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmStatus {
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            state: Arc::new(AtomicU8::new(ServerState::Unknown as u8)),
            provider: Arc::new(RwLock::new(None)),
            url: Arc::new(RwLock::new(None)),
            models: Arc::new(RwLock::new(Vec::new())),
            active_model: Arc::new(RwLock::new(None)),
            omlx_installed: Arc::new(AtomicBool::new(false)),
            omlx_running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn server_state(&self) -> ServerState {
        self.state.load(Ordering::Acquire).into()
    }

    pub fn set_server_state(&self, s: ServerState) {
        self.state.store(s as u8, Ordering::Release);
    }

    pub fn active_model(&self) -> Option<String> {
        self.active_model.read().unwrap().clone()
    }

    /// Shared reference for LlmClient to read at request time.
    pub fn active_model_ref(&self) -> Arc<RwLock<Option<String>>> {
        self.active_model.clone()
    }

    pub fn set_connection(
        &self,
        provider: Provider,
        url: String,
        models: Vec<String>,
        active: Option<String>,
    ) {
        *self.provider.write().unwrap() = Some(provider);
        *self.url.write().unwrap() = Some(url);
        *self.models.write().unwrap() = models;
        *self.active_model.write().unwrap() = active;
        self.ready.store(true, Ordering::Release);
        self.set_server_state(ServerState::Connected);
    }

    pub fn clear_connection(&self) {
        *self.provider.write().unwrap() = None;
        *self.url.write().unwrap() = None;
        self.models.write().unwrap().clear();
        *self.active_model.write().unwrap() = None;
        self.ready.store(false, Ordering::Release);
    }

    pub fn set_omlx_installed(&self, v: bool) {
        self.omlx_installed.store(v, Ordering::Release);
    }

    pub fn set_omlx_running(&self, v: bool) {
        self.omlx_running.store(v, Ordering::Release);
    }

    pub fn snapshot(&self) -> StatusSnapshot {
        StatusSnapshot {
            enabled: false, // overridden by service.status_snapshot()
            state: self.server_state(),
            provider: *self.provider.read().unwrap(),
            url: self.url.read().unwrap().clone(),
            models: self.models.read().unwrap().clone(),
            active_model: self.active_model.read().unwrap().clone(),
            classify_mode: ClassifyMode::default(), // overridden by service
            omlx_installed: self.omlx_installed.load(Ordering::Acquire),
            omlx_running: self.omlx_running.load(Ordering::Acquire),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusSnapshot {
    pub enabled: bool,
    pub state: ServerState,
    pub provider: Option<Provider>,
    pub url: Option<String>,
    pub models: Vec<String>,
    pub active_model: Option<String>,
    pub classify_mode: ClassifyMode,
    pub omlx_installed: bool,
    pub omlx_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_status_is_unknown_and_not_ready() {
        let s = LlmStatus::new();
        assert!(!s.ready.load(Ordering::Acquire));
        assert_eq!(s.server_state(), ServerState::Unknown);
        assert!(s.active_model().is_none());
    }

    #[test]
    fn set_connection_makes_ready() {
        let s = LlmStatus::new();
        s.set_connection(
            Provider::Ollama,
            "http://localhost:11434".into(),
            vec!["llama3.2".into()],
            Some("llama3.2".into()),
        );
        assert!(s.ready.load(Ordering::Acquire));
        assert_eq!(s.server_state(), ServerState::Connected);
        assert_eq!(s.active_model().as_deref(), Some("llama3.2"));
    }

    #[test]
    fn clear_connection_resets_all() {
        let s = LlmStatus::new();
        s.set_connection(
            Provider::Omlx,
            "http://localhost:10710".into(),
            vec!["qwen".into()],
            Some("qwen".into()),
        );
        s.clear_connection();
        assert!(!s.ready.load(Ordering::Acquire));
        assert!(s.active_model().is_none());

        let snap = s.snapshot();
        assert!(snap.provider.is_none());
        assert!(snap.models.is_empty());
    }

    #[test]
    fn snapshot_reflects_current_state() {
        let s = LlmStatus::new();
        s.set_connection(
            Provider::Ollama,
            "http://localhost:11434".into(),
            vec!["model-a".into(), "model-b".into()],
            Some("model-a".into()),
        );
        s.set_omlx_installed(true);
        s.set_omlx_running(false);

        let snap = s.snapshot();
        assert_eq!(snap.state, ServerState::Connected);
        assert_eq!(snap.provider, Some(Provider::Ollama));
        assert_eq!(snap.models.len(), 2);
        assert!(snap.omlx_installed);
        assert!(!snap.omlx_running);
    }
}
