//! Atomic git-sync progress state for lock-free SSE streaming.
//!
//! [`GitSyncState`] uses atomics so the git-sync background task can update
//! progress counters while the SSE endpoint reads them without contention.
//!
//! **Key divergence from [`super::indexing_state::IndexingState`]:**
//! `IndexingState` has no `reset()` — it's created once per server start and
//! runs a single indexing pass. `GitSyncState` needs `reset()` because users
//! trigger multiple syncs via `POST /api/sync/git` without restarting the
//! server. The same `Arc<GitSyncState>` lives in `AppState` for the server's
//! lifetime, and `reset()` clears stale counters before each new sync.

use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::RwLock;

/// Which phase the git-sync operation is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GitSyncPhase {
    /// No sync in progress.
    Idle = 0,
    /// Scanning repos for commits.
    Scanning = 1,
    /// Linking commits to sessions.
    Correlating = 2,
    /// Sync finished successfully.
    Done = 3,
    /// Sync terminated with an error (see [`GitSyncState::error`]).
    Error = 4,
}

impl GitSyncPhase {
    /// Convert a raw `u8` into a phase variant.
    /// Returns `None` for values outside the valid range.
    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Idle),
            1 => Some(Self::Scanning),
            2 => Some(Self::Correlating),
            3 => Some(Self::Done),
            4 => Some(Self::Error),
            _ => None,
        }
    }
}

/// Thread-safe, lock-free progress state for the background git-sync task.
///
/// All numeric counters use [`Ordering::Relaxed`] – we only need
/// monotonically-increasing values visible *eventually* to readers,
/// not cross-field consistency.
pub struct GitSyncState {
    phase: AtomicU8,
    repos_scanned: AtomicUsize,
    total_repos: AtomicUsize,
    commits_found: AtomicUsize,
    sessions_correlated: AtomicUsize,
    total_correlatable_sessions: AtomicUsize,
    links_created: AtomicUsize,
    error: RwLock<Option<String>>,
}

impl GitSyncState {
    /// Create a new state initialised to [`GitSyncPhase::Idle`] with all
    /// counters at zero.
    pub fn new() -> Self {
        Self {
            phase: AtomicU8::new(GitSyncPhase::Idle as u8),
            repos_scanned: AtomicUsize::new(0),
            total_repos: AtomicUsize::new(0),
            commits_found: AtomicUsize::new(0),
            sessions_correlated: AtomicUsize::new(0),
            total_correlatable_sessions: AtomicUsize::new(0),
            links_created: AtomicUsize::new(0),
            error: RwLock::new(None),
        }
    }

    /// Reset all counters and phase to initial state.
    ///
    /// Must be called before each new sync to clear stale values from the
    /// previous run. Safe to call from any thread.
    pub fn reset(&self) {
        self.phase.store(GitSyncPhase::Idle as u8, Ordering::Relaxed);
        self.repos_scanned.store(0, Ordering::Relaxed);
        self.total_repos.store(0, Ordering::Relaxed);
        self.commits_found.store(0, Ordering::Relaxed);
        self.sessions_correlated.store(0, Ordering::Relaxed);
        self.total_correlatable_sessions.store(0, Ordering::Relaxed);
        self.links_created.store(0, Ordering::Relaxed);
        if let Ok(mut guard) = self.error.write() {
            *guard = None;
        }
    }

    // -- Phase ----------------------------------------------------------------

    /// Current sync phase.
    pub fn phase(&self) -> GitSyncPhase {
        let raw = self.phase.load(Ordering::Relaxed);
        GitSyncPhase::from_u8(raw).unwrap_or(GitSyncPhase::Error)
    }

    /// Set the sync phase.
    pub fn set_phase(&self, phase: GitSyncPhase) {
        self.phase.store(phase as u8, Ordering::Relaxed);
    }

    // -- Counters (store-based) -----------------------------------------------

    /// Number of repos scanned so far.
    pub fn repos_scanned(&self) -> usize {
        self.repos_scanned.load(Ordering::Relaxed)
    }

    /// Set the number of repos scanned.
    pub fn set_repos_scanned(&self, val: usize) {
        self.repos_scanned.store(val, Ordering::Relaxed);
    }

    /// Total number of repos to scan.
    pub fn total_repos(&self) -> usize {
        self.total_repos.load(Ordering::Relaxed)
    }

    /// Set the total number of repos to scan.
    pub fn set_total_repos(&self, val: usize) {
        self.total_repos.store(val, Ordering::Relaxed);
    }

    /// Number of sessions correlated so far.
    pub fn sessions_correlated(&self) -> usize {
        self.sessions_correlated.load(Ordering::Relaxed)
    }

    /// Set the number of sessions correlated.
    pub fn set_sessions_correlated(&self, val: usize) {
        self.sessions_correlated.store(val, Ordering::Relaxed);
    }

    /// Total number of sessions eligible for correlation.
    pub fn total_correlatable_sessions(&self) -> usize {
        self.total_correlatable_sessions.load(Ordering::Relaxed)
    }

    /// Set the total number of sessions eligible for correlation.
    pub fn set_total_correlatable_sessions(&self, val: usize) {
        self.total_correlatable_sessions.store(val, Ordering::Relaxed);
    }

    // -- Counters (fetch_add-based) -------------------------------------------

    /// Total commits found across all repos.
    pub fn commits_found(&self) -> usize {
        self.commits_found.load(Ordering::Relaxed)
    }

    /// Add to the commits-found counter (accumulates across repos).
    /// Returns the **previous** value before the add.
    pub fn add_commits_found(&self, count: usize) -> usize {
        self.commits_found.fetch_add(count, Ordering::Relaxed)
    }

    /// Total session-commit links created.
    pub fn links_created(&self) -> usize {
        self.links_created.load(Ordering::Relaxed)
    }

    /// Add to the links-created counter (accumulates across sessions).
    /// Returns the **previous** value before the add.
    pub fn add_links_created(&self, count: usize) -> usize {
        self.links_created.fetch_add(count, Ordering::Relaxed)
    }

    // -- Error ----------------------------------------------------------------

    /// Record an error message (also sets phase to [`GitSyncPhase::Error`]).
    pub fn set_error(&self, msg: String) {
        self.set_phase(GitSyncPhase::Error);
        if let Ok(mut guard) = self.error.write() {
            *guard = Some(msg);
        }
    }

    /// Retrieve the current error message, if any.
    pub fn error(&self) -> Option<String> {
        self.error.read().ok().and_then(|g| g.clone())
    }
}

impl Default for GitSyncState {
    fn default() -> Self {
        Self::new()
    }
}
