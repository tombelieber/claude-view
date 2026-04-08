//! Global mutexes to prevent concurrent sync operations.

use tokio::sync::Mutex;

/// Global mutex to prevent concurrent git syncs.
/// Uses a lazy static pattern via std::sync::OnceLock.
static GIT_SYNC_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

/// Global mutex to prevent concurrent deep index rebuilds.
static DEEP_INDEX_MUTEX: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

pub fn get_sync_mutex() -> &'static Mutex<()> {
    GIT_SYNC_MUTEX.get_or_init(|| Mutex::new(()))
}

pub fn get_deep_index_mutex() -> &'static Mutex<()> {
    DEEP_INDEX_MUTEX.get_or_init(|| Mutex::new(()))
}
