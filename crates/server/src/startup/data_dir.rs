//! Data directory validation + legacy artefact cleanup.
//!
//! Extracted from `main.rs` in CQRS Phase 7.f. Side-effects and error
//! messages are unchanged so existing ops runbooks still apply.

use std::path::PathBuf;

use claude_view_db::Database;

/// Validate the data directory is creatable and writable. On failure, print
/// a helpful error and exit with status 1. Returns the resolved data dir on
/// success.
///
/// Also fires the one-shot best-effort cleanup of Phase-1 legacy pairing
/// artefacts (`paired-devices.json`, `pairing_secrets/`) — Supabase is the
/// authority for device list post-Phase-2.
pub fn validate_and_cleanup_legacy() -> PathBuf {
    let data_dir = claude_view_core::paths::data_dir();
    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        eprintln!(
            "ERROR: Cannot create data directory: {}\n\
             Path: {}\n\
             Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
            e,
            data_dir.display()
        );
        std::process::exit(1);
    }

    crate::crypto::cleanup_legacy_pairing_artifacts();

    let probe = data_dir.join(".write-test");
    if std::fs::write(&probe, b"ok").is_err() {
        eprintln!(
            "ERROR: Data directory is not writable: {}\n\
             Set CLAUDE_VIEW_DATA_DIR to a writable directory.",
            data_dir.display()
        );
        std::process::exit(1);
    }
    let _ = std::fs::remove_file(&probe);
    tracing::info!("Data directory: {}", data_dir.display());
    data_dir
}

/// Recover any classification jobs left in "running" state from a previous
/// crash. Non-fatal — logs and continues.
pub async fn recover_stale_classification_jobs(db: &Database) {
    match db.recover_stale_classification_jobs().await {
        Ok(count) if count > 0 => {
            tracing::info!("Recovered {} stale classification jobs", count);
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!("Failed to recover stale classification jobs: {}", e);
        }
    }
}
