use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use serde::Serialize;

/// Explicit server state — replaces implicit ready=true/false.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    Unknown = 0,
    Running = 1,
    Unavailable = 2,
}

impl From<u8> for ServerState {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Running,
            2 => Self::Unavailable,
            _ => Self::Unknown,
        }
    }
}

/// Lock-free shared status for the local LLM process.
#[derive(Debug)]
pub struct LlmStatus {
    pub ready: Arc<AtomicBool>,
    pub port: u16,
    pid: Arc<AtomicU32>,
    state: Arc<AtomicU8>,
}

impl LlmStatus {
    pub fn new(port: u16) -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
            port,
            pid: Arc::new(AtomicU32::new(0)),
            state: Arc::new(AtomicU8::new(ServerState::Unknown as u8)),
        }
    }

    pub fn pid(&self) -> Option<u32> {
        match self.pid.load(Ordering::Acquire) {
            0 => None,
            v => Some(v),
        }
    }

    pub fn set_pid(&self, pid: Option<u32>) {
        self.pid.store(pid.unwrap_or(0), Ordering::Release);
    }

    pub fn server_state(&self) -> ServerState {
        self.state.load(Ordering::Acquire).into()
    }

    pub fn set_server_state(&self, s: ServerState) {
        self.state.store(s as u8, Ordering::Release);
    }

    /// Snapshot for the status route — cheap, lock-free.
    pub fn snapshot(&self) -> StatusSnapshot {
        StatusSnapshot {
            ready: self.ready.load(Ordering::Acquire),
            port: self.port,
            pid: self.pid(),
            state: self.server_state(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct StatusSnapshot {
    pub ready: bool,
    pub port: u16,
    pub pid: Option<u32>,
    pub state: ServerState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_status_is_unknown_and_not_ready() {
        let s = LlmStatus::new(10710);
        assert!(!s.ready.load(Ordering::Acquire));
        assert_eq!(s.pid(), None);
        assert_eq!(s.server_state(), ServerState::Unknown);
    }

    #[test]
    fn pid_round_trip() {
        let s = LlmStatus::new(10710);
        s.set_pid(Some(42));
        assert_eq!(s.pid(), Some(42));
        s.set_pid(None);
        assert_eq!(s.pid(), None);
    }

    #[test]
    fn state_transitions() {
        let s = LlmStatus::new(10710);
        s.set_server_state(ServerState::Running);
        assert_eq!(s.server_state(), ServerState::Running);
        s.set_server_state(ServerState::Unavailable);
        assert_eq!(s.server_state(), ServerState::Unavailable);
    }

    #[test]
    fn snapshot_reflects_current_state() {
        let s = LlmStatus::new(10710);
        s.ready.store(true, Ordering::Release);
        s.set_pid(Some(1234));
        s.set_server_state(ServerState::Running);

        let snap = s.snapshot();
        assert!(snap.ready);
        assert_eq!(snap.pid, Some(1234));
        assert_eq!(snap.state, ServerState::Running);
        assert_eq!(snap.port, 10710);
    }
}
