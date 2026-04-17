//! Unit tests for the Supabase auth-session persistence layer.
//!
//! All tests run against a temp CLAUDE_VIEW_DATA_DIR so they never pollute
//! the user's real ~/.claude-view/.

use claude_view_server::auth::session_store::{AuthSession, SessionStore};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;

fn unix_now_plus(seconds: u64) -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
        + seconds
}

fn sample_session() -> AuthSession {
    AuthSession {
        user_id: "00000000-0000-0000-0000-000000000001".to_string(),
        email: Some("user@example.com".to_string()),
        access_token: "eyJhbGciOiJIUzI1NiJ9.test.sig".to_string(),
        refresh_token: "rft_test_0123456789abcdef".to_string(),
        expires_at_unix: unix_now_plus(3600),
    }
}

#[tokio::test]
async fn store_and_load_round_trip() {
    let dir = tempdir().expect("tempdir");
    let store = SessionStore::with_path(dir.path().join("auth-session.json"));
    let session = sample_session();
    store.save(&session).await.expect("save");

    let loaded = store.load().await.expect("load");
    let loaded = loaded.expect("session present");
    assert_eq!(loaded.user_id, session.user_id);
    assert_eq!(loaded.email, session.email);
    assert_eq!(loaded.access_token, session.access_token);
    assert_eq!(loaded.refresh_token, session.refresh_token);
    assert_eq!(loaded.expires_at_unix, session.expires_at_unix);
}

#[tokio::test]
async fn load_returns_none_when_missing() {
    let dir = tempdir().expect("tempdir");
    let store = SessionStore::with_path(dir.path().join("auth-session.json"));
    let loaded = store.load().await.expect("load");
    assert!(loaded.is_none());
}

#[tokio::test]
async fn corrupt_file_returns_err_does_not_panic() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("auth-session.json");
    std::fs::write(&path, b"not-json").unwrap();

    let store = SessionStore::with_path(path);
    let loaded = store.load().await;
    assert!(
        loaded.is_err(),
        "corrupt auth-session.json must surface an error rather than panic or silently return None"
    );
}

#[tokio::test]
async fn clear_deletes_file() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("auth-session.json");
    let store = SessionStore::with_path(path.clone());
    store.save(&sample_session()).await.unwrap();
    assert!(path.exists());

    store.clear().await.expect("clear");
    assert!(!path.exists());
    assert!(store.load().await.unwrap().is_none());
}

#[tokio::test]
async fn is_near_expiry_returns_true_when_within_window() {
    let mut s = sample_session();
    s.expires_at_unix = unix_now_plus(60); // 60s remain
    assert!(s.is_near_expiry(std::time::Duration::from_secs(500)));
}

#[tokio::test]
async fn is_near_expiry_returns_false_when_fresh() {
    let mut s = sample_session();
    s.expires_at_unix = unix_now_plus(3600); // 1h remains
    assert!(!s.is_near_expiry(std::time::Duration::from_secs(500)));
}

#[tokio::test]
async fn defaults_fill_missing_fields_on_deserialize() {
    // Per feedback_external_data_serde_default.md — auth-session.json must
    // tolerate forward-compatible additions without dropping the file.
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("auth-session.json");
    // Minimal JSON — intentionally missing `email`.
    std::fs::write(
        &path,
        br#"{"user_id":"u","access_token":"a","refresh_token":"r","expires_at_unix":1}"#,
    )
    .unwrap();

    let store = SessionStore::with_path(path);
    let loaded = store.load().await.expect("load").expect("some");
    assert_eq!(loaded.user_id, "u");
    assert!(loaded.email.is_none(), "missing email must default to None");
}
