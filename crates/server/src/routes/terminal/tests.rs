use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite;

use crate::state::AppState;

use super::format::{format_line_for_mode, strip_command_tags};
use super::router;
use super::types::RichModeFinders;

/// Helper: create an AppState with an in-memory database and a live session
/// registered pointing to the given JSONL file path.
async fn test_state_with_session(session_id: &str, file_path: &str) -> Arc<AppState> {
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = Arc::new(AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing: Arc::new(crate::IndexingState::new()),
        registry: Arc::new(std::sync::RwLock::new(None)),
        jobs: Arc::new(crate::jobs::JobRunner::new()),
        classify: Arc::new(crate::classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
        git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
        pricing: Arc::new(std::collections::HashMap::new()),
        live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        recently_closed: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        live_tx: tokio::sync::broadcast::channel(256).0,

        rules_dir: std::env::temp_dir().join("claude-rules-test"),
        terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
        live_manager: None,
        search_index: Arc::new(std::sync::RwLock::new(None)),
        shutdown: tokio::sync::watch::channel(false).1,
        hook_event_channels: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        sidecar: Arc::new(crate::sidecar::SidecarManager::new()),
        jwks: None,
        share: None,
        auth_identity: tokio::sync::OnceCell::new(),
        oauth_usage_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        plugin_cli_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        teams: Arc::new(crate::teams::TeamsStore::empty()),
        prompt_index: Arc::new(std::sync::RwLock::new(None)),
        prompt_stats: Arc::new(std::sync::RwLock::new(None)),
        prompt_templates: Arc::new(std::sync::RwLock::new(None)),
        available_ides: Vec::new(),
        monitor_tx: tokio::sync::broadcast::channel(64).0,
        monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        oracle_rx: crate::live::process_oracle::stub(),
        plugin_op_queue: Arc::new(crate::routes::plugin_ops::PluginOpQueue::new()),
        plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
        marketplace_refresh: Arc::new(
            crate::routes::marketplace_refresh::MarketplaceRefreshTracker::new(),
        ),
        transcript_to_session: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        pending_statusline: tokio::sync::Mutex::new(crate::live::buffer::PendingMutations::new(
            std::time::Duration::from_secs(120),
        )),
        coordinator: std::sync::Arc::new(crate::live::coordinator::SessionCoordinator::new()),
        telemetry: None,
        telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
        debug_statusline_log: None,
        debug_hooks_log: None,
        debug_omlx_log: None,
        local_llm: Arc::new(crate::local_llm::LocalLlmService::new(
            Arc::new(crate::local_llm::LocalLlmConfig::new_disabled()),
            Arc::new(crate::local_llm::LlmStatus::new()),
        )),
        session_channels: Arc::new(
            crate::live::session_ws::registry::SessionChannelRegistry::new(),
        ),
        api_key_store: Arc::new(tokio::sync::RwLock::new(
            crate::auth::api_key::ApiKeyStore::default(),
        )),
        api_key_store_path: std::env::temp_dir().join("api-keys.json"),
        webhook_config_path: std::env::temp_dir().join("notifications.json"),
        webhook_secrets_path: std::env::temp_dir().join("webhook-secrets.json"),
        cli_sessions: Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new()),
        tmux: Arc::new(crate::routes::cli_sessions::tmux::RealTmux),
    });

    // Register the session in the live sessions map
    {
        let mut map = state.live_sessions.write().await;
        let session = crate::live::state::LiveSession {
            id: session_id.to_string(),
            status: crate::live::state::SessionStatus::Working,
            started_at: None,
            closed_at: None,
            control: None,
            model: None,
            model_display_name: None,
            model_set_at: 0,
            context_window_tokens: 0,
            statusline: crate::live::state::StatuslineFields::default(),
            hook: crate::live::state::HookFields {
                agent_state: crate::live::state::AgentState {
                    group: crate::live::state::AgentStateGroup::Autonomous,
                    state: "working".to_string(),
                    label: "Working".to_string(),
                    context: None,
                },
                pid: None,
                title: "Test session".to_string(),
                last_user_message: "test".to_string(),
                current_activity: "testing".to_string(),
                turn_count: 0,
                last_activity_at: 0,
                current_turn_started_at: None,
                sub_agents: Vec::new(),
                progress_items: Vec::new(),
                compact_count: 0,
                agent_state_set_at: 0,
                hook_events: Vec::new(),
                last_assistant_preview: None,
                last_error: None,
                last_error_details: None,
            },
            jsonl: crate::live::state::JsonlFields {
                project: "test-project".to_string(),
                project_display_name: "test-project".to_string(),
                project_path: "/tmp/test-project".to_string(),
                file_path: file_path.to_string(),
                ..crate::live::state::JsonlFields::default()
            },
            session_kind: None,
            entrypoint: None,
        };
        map.insert(session_id.to_string(), session);
    }

    state
}

/// Helper: start an Axum server on a random port, returning the bound address.
/// The server runs as a background task that is cancelled when the returned
/// `JoinHandle` is aborted.
async fn start_test_server(
    state: Arc<AppState>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new().nest("/api/live", router()).with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (addr, handle)
}

/// Helper: connect a WebSocket client to the test server.
async fn ws_connect(
    addr: std::net::SocketAddr,
    session_id: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!(
        "ws://127.0.0.1:{}/api/live/sessions/{}/terminal",
        addr.port(),
        session_id
    );
    let (ws_stream, _response) = tokio_tungstenite::connect_async(&url).await.unwrap();
    ws_stream
}

/// Helper: receive a text message with a timeout.
async fn recv_text(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<String> {
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(tungstenite::Message::Text(text)))) => Some(text.to_string()),
        _ => None,
    }
}

/// Helper: receive text messages until we find one matching the given type.
async fn recv_until_type(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    target_type: &str,
) -> Option<serde_json::Value> {
    for _ in 0..50 {
        if let Some(text) = recv_text(ws).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if v.get("type").and_then(|t| t.as_str()) == Some(target_type) {
                    return Some(v);
                }
            }
        } else {
            return None;
        }
    }
    None
}

// =========================================================================
// Test 1: ws_upgrade_returns_101
// =========================================================================

#[tokio::test]
async fn ws_upgrade_returns_101() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
    )
    .unwrap();

    let state = test_state_with_session("test-ws-upgrade", tmp.path().to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;

    // Connecting successfully means we got a 101 Switching Protocols response.
    // tokio-tungstenite would error if the upgrade failed.
    let mut ws = ws_connect(addr, "test-ws-upgrade").await;

    // Send handshake and verify we get a response
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"raw","scrollback":10}"#.into(),
    ))
    .await
    .unwrap();

    // Should receive at least one message (scrollback line or buffer_end)
    let msg = recv_text(&mut ws).await;
    assert!(
        msg.is_some(),
        "Expected at least one message after handshake"
    );

    ws.close(None).await.ok();
    server_handle.abort();
}

// =========================================================================
// Test 2: ws_unknown_session_returns_error
// =========================================================================

#[tokio::test]
async fn ws_unknown_session_returns_error() {
    // Create state with NO sessions registered
    let db = claude_view_db::Database::new_in_memory().await.unwrap();
    let state = Arc::new(AppState {
        start_time: std::time::Instant::now(),
        db,
        indexing: Arc::new(crate::IndexingState::new()),
        registry: Arc::new(std::sync::RwLock::new(None)),
        jobs: Arc::new(crate::jobs::JobRunner::new()),
        classify: Arc::new(crate::classify_state::ClassifyState::new()),
        facet_ingest: Arc::new(crate::facet_ingest::FacetIngestState::new()),
        git_sync: Arc::new(crate::git_sync_state::GitSyncState::new()),
        pricing: Arc::new(std::collections::HashMap::new()),
        live_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        recently_closed: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        live_tx: tokio::sync::broadcast::channel(256).0,

        rules_dir: std::env::temp_dir().join("claude-rules-test"),
        terminal_connections: Arc::new(crate::terminal_state::TerminalConnectionManager::new()),
        live_manager: None,
        search_index: Arc::new(std::sync::RwLock::new(None)),
        shutdown: tokio::sync::watch::channel(false).1,
        hook_event_channels: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        sidecar: Arc::new(crate::sidecar::SidecarManager::new()),
        jwks: None,
        share: None,
        auth_identity: tokio::sync::OnceCell::new(),
        oauth_usage_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        plugin_cli_cache: crate::cache::CachedUpstream::new(std::time::Duration::from_secs(300)),
        teams: Arc::new(crate::teams::TeamsStore::empty()),
        prompt_index: Arc::new(std::sync::RwLock::new(None)),
        prompt_stats: Arc::new(std::sync::RwLock::new(None)),
        prompt_templates: Arc::new(std::sync::RwLock::new(None)),
        available_ides: Vec::new(),
        monitor_tx: tokio::sync::broadcast::channel(64).0,
        monitor_subscribers: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        oracle_rx: crate::live::process_oracle::stub(),
        plugin_op_queue: Arc::new(crate::routes::plugin_ops::PluginOpQueue::new()),
        plugin_op_notify: Arc::new(tokio::sync::Notify::new()),
        marketplace_refresh: Arc::new(
            crate::routes::marketplace_refresh::MarketplaceRefreshTracker::new(),
        ),
        transcript_to_session: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        pending_statusline: tokio::sync::Mutex::new(crate::live::buffer::PendingMutations::new(
            std::time::Duration::from_secs(120),
        )),
        coordinator: std::sync::Arc::new(crate::live::coordinator::SessionCoordinator::new()),
        telemetry: None,
        telemetry_config_path: claude_view_core::telemetry_config::telemetry_config_path(),
        debug_statusline_log: None,
        debug_hooks_log: None,
        debug_omlx_log: None,
        local_llm: Arc::new(crate::local_llm::LocalLlmService::new(
            Arc::new(crate::local_llm::LocalLlmConfig::new_disabled()),
            Arc::new(crate::local_llm::LlmStatus::new()),
        )),
        session_channels: Arc::new(
            crate::live::session_ws::registry::SessionChannelRegistry::new(),
        ),
        api_key_store: Arc::new(tokio::sync::RwLock::new(
            crate::auth::api_key::ApiKeyStore::default(),
        )),
        api_key_store_path: std::env::temp_dir().join("api-keys.json"),
        webhook_config_path: std::env::temp_dir().join("notifications.json"),
        webhook_secrets_path: std::env::temp_dir().join("webhook-secrets.json"),
        cli_sessions: Arc::new(crate::routes::cli_sessions::store::CliSessionStore::new()),
        tmux: Arc::new(crate::routes::cli_sessions::tmux::RealTmux),
    });

    let (addr, server_handle) = start_test_server(state).await;
    let mut ws = ws_connect(addr, "nonexistent-session-id").await;

    // The server should send an error message and close
    let msg = recv_text(&mut ws).await;
    assert!(msg.is_some(), "Expected error message");

    let parsed: serde_json::Value = serde_json::from_str(&msg.unwrap()).unwrap();
    assert_eq!(parsed["type"], "error");
    assert!(
        parsed["message"].as_str().unwrap().contains("not found"),
        "Error message should mention 'not found'"
    );

    // Should receive a close frame next
    match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
        Ok(Some(Ok(tungstenite::Message::Close(frame)))) => {
            if let Some(cf) = frame {
                assert_eq!(
                    cf.code,
                    tungstenite::protocol::frame::coding::CloseCode::from(4004)
                );
            }
        }
        _ => {
            // Connection may already be closed -- that's acceptable
        }
    }

    server_handle.abort();
}

// =========================================================================
// Test 3: ws_initial_buffer_sent
// =========================================================================

#[tokio::test]
async fn ws_initial_buffer_sent() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Write 3 JSONL lines
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","message":{{"role":"user","content":"line 1"}}}}"#
    )
    .unwrap();
    writeln!(
        tmp.as_file(),
        r#"{{"type":"assistant","message":{{"role":"assistant","content":"line 2"}}}}"#
    )
    .unwrap();
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","message":{{"role":"user","content":"line 3"}}}}"#
    )
    .unwrap();
    tmp.as_file().flush().unwrap();

    let state = test_state_with_session("test-buffer", tmp.path().to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;

    let mut ws = ws_connect(addr, "test-buffer").await;

    // Send handshake requesting all 3 scrollback lines
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"raw","scrollback":10}"#.into(),
    ))
    .await
    .unwrap();

    // Collect messages until buffer_end
    let mut lines = Vec::new();
    let mut found_buffer_end = false;
    for _ in 0..20 {
        if let Some(text) = recv_text(&mut ws).await {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
            match parsed["type"].as_str() {
                Some("line") => lines.push(parsed),
                Some("buffer_end") => {
                    found_buffer_end = true;
                    break;
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    assert!(found_buffer_end, "Expected buffer_end marker");
    assert_eq!(
        lines.len(),
        3,
        "Expected 3 scrollback lines, got {}",
        lines.len()
    );

    // Verify lines contain the original data
    assert!(lines[0]["data"].as_str().unwrap().contains("line 1"));
    assert!(lines[1]["data"].as_str().unwrap().contains("line 2"));
    assert!(lines[2]["data"].as_str().unwrap().contains("line 3"));

    ws.close(None).await.ok();
    server_handle.abort();
}

// =========================================================================
// Test 4: ws_live_lines_streamed
// =========================================================================

#[tokio::test]
async fn ws_live_lines_streamed() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Write initial content
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","message":{{"role":"user","content":"initial"}}}}"#
    )
    .unwrap();
    tmp.as_file().flush().unwrap();

    let state = test_state_with_session("test-live", tmp.path().to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;

    let mut ws = ws_connect(addr, "test-live").await;

    // Send handshake
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"raw","scrollback":10}"#.into(),
    ))
    .await
    .unwrap();

    // Wait for buffer_end
    let _buffer_end = recv_until_type(&mut ws, "buffer_end").await;
    assert!(_buffer_end.is_some(), "Expected buffer_end");

    // Append new lines to the file in a loop to reliably trigger the
    // file watcher (macOS FSEvents can batch/coalesce events).
    let path = tmp.path().to_path_buf();
    let write_path = path.clone();
    let write_handle = tokio::spawn(async move {
        // Write the target line, then keep poking the file to ensure
        // the watcher fires. On macOS FSEvents, a single small write
        // may not reliably trigger a notification within the test timeout.
        for i in 0..10 {
            {
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&write_path)
                    .unwrap();
                if i == 0 {
                    writeln!(f, r#"{{"type":"assistant","message":{{"role":"assistant","content":"live response"}}}}"#).unwrap();
                } else {
                    writeln!(
                        f,
                        r#"{{"type":"system","message":{{"role":"system","content":"poke {i}"}}}}"#
                    )
                    .unwrap();
                }
                f.flush().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });

    // Wait for the live line to arrive (file watcher + debounce delay).
    // Use a generous outer timeout and keep looping even when individual
    // recv_text calls time out — on macOS the FSEvents watcher may take
    // several seconds to coalesce and fire.
    let live_msg = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(text) = recv_text(&mut ws).await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v["type"] == "line"
                        && v["data"].as_str().unwrap_or("").contains("live response")
                    {
                        return v;
                    }
                }
            }
            // recv_text returned None (per-message timeout) — keep waiting
            // for the outer timeout to expire rather than giving up early.
        }
    })
    .await;

    write_handle.abort();

    assert!(
        live_msg.is_ok(),
        "Expected live streamed line containing 'live response'"
    );

    ws.close(None).await.ok();
    server_handle.abort();
}

// =========================================================================
// Test 5: ws_disconnect_drops_watcher
// =========================================================================

#[tokio::test]
async fn ws_disconnect_drops_watcher() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","message":{{"role":"user","content":"hello"}}}}"#
    )
    .unwrap();
    tmp.as_file().flush().unwrap();

    let state = test_state_with_session("test-disconnect", tmp.path().to_str().unwrap()).await;
    let terminal_connections = state.terminal_connections.clone();

    let (addr, server_handle) = start_test_server(state).await;

    // Connect and do handshake
    let mut ws = ws_connect(addr, "test-disconnect").await;
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"raw","scrollback":1}"#.into(),
    ))
    .await
    .unwrap();

    // Wait for buffer_end
    let _ = recv_until_type(&mut ws, "buffer_end").await;

    // Verify connection is tracked
    // Allow a small delay for the server to register the connection
    tokio::time::sleep(Duration::from_millis(100)).await;
    let count_before = terminal_connections.viewer_count("test-disconnect");
    assert_eq!(count_before, 1, "Expected 1 viewer before disconnect");

    // Disconnect
    ws.close(None).await.ok();

    // Wait for the server to process the disconnect
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify connection count decremented
    let count_after = terminal_connections.viewer_count("test-disconnect");
    assert_eq!(count_after, 0, "Expected 0 viewers after disconnect");

    server_handle.abort();
}

// =========================================================================
// Test 6: ws_mode_switch
// =========================================================================

#[tokio::test]
async fn ws_mode_switch() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    // Write a JSONL line with structured content
    writeln!(
        tmp.as_file(),
        r#"{{"type":"assistant","message":{{"role":"assistant","content":"initial data"}}}}"#
    )
    .unwrap();
    tmp.as_file().flush().unwrap();

    let state = test_state_with_session("test-mode-switch", tmp.path().to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;

    let mut ws = ws_connect(addr, "test-mode-switch").await;

    // Start in raw mode
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"raw","scrollback":1}"#.into(),
    ))
    .await
    .unwrap();

    // Wait for buffer_end -- the scrollback lines should be in raw format
    let mut raw_lines = Vec::new();
    for _ in 0..10 {
        if let Some(text) = recv_text(&mut ws).await {
            let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
            match parsed["type"].as_str() {
                Some("line") => {
                    // Raw mode: should have "data" field with the original line
                    assert!(
                        parsed.get("data").is_some(),
                        "Raw mode should have 'data' field"
                    );
                    raw_lines.push(parsed);
                }
                Some("buffer_end") => break,
                _ => {}
            }
        }
    }
    assert!(
        !raw_lines.is_empty(),
        "Should have received at least 1 raw line"
    );

    // Switch to rich mode
    ws.send(tungstenite::Message::Text(
        r#"{"type":"mode","mode":"rich"}"#.into(),
    ))
    .await
    .unwrap();

    // Small delay to let the mode switch be processed
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Append new lines to reliably trigger the file watcher
    let path = tmp.path().to_path_buf();
    let write_path = path.clone();
    let write_handle = tokio::spawn(async move {
        for i in 0..10 {
            {
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&write_path)
                    .unwrap();
                if i == 0 {
                    writeln!(
                        f,
                        r#"{{"type":"assistant","message":{{"role":"assistant","content":[{{"type":"text","text":"rich content here"}}]}},"timestamp":"2026-01-15T10:30:00Z"}}"#
                    )
                    .unwrap();
                } else {
                    writeln!(
                        f,
                        r#"{{"type":"system","message":{{"role":"system","content":"poke {i}"}}}}"#
                    )
                    .unwrap();
                }
                f.flush().unwrap();
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
    });

    // Wait for the rich-mode message.
    // Keep looping even when individual recv_text calls time out — on
    // macOS the FSEvents watcher may take several seconds to fire.
    let rich_msg = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            if let Some(text) = recv_text(&mut ws).await {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    // In rich mode, the type should be "message" (not "line")
                    if v["type"] == "message" {
                        return v;
                    }
                }
            }
            // recv_text returned None (per-message timeout) — keep waiting
            // for the outer timeout to expire rather than giving up early.
        }
    })
    .await;

    write_handle.abort();

    assert!(rich_msg.is_ok(), "Timed out waiting for rich mode message");

    let msg = rich_msg.unwrap();
    assert_eq!(msg["type"], "message");
    assert_eq!(msg["role"], "assistant");
    assert!(
        msg.get("content").is_some(),
        "Rich mode message should have content"
    );
    assert_eq!(msg["ts"], "2026-01-15T10:30:00Z");

    ws.close(None).await.ok();
    server_handle.abort();
}

// =========================================================================
// Unit tests for format_line_for_mode
// =========================================================================

#[test]
fn format_line_raw_mode() {
    let finders = RichModeFinders::new();
    let results = format_line_for_mode("some raw line", "raw", &finders);
    assert_eq!(results.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "line");
    assert_eq!(parsed["data"], "some raw line");
}

#[test]
fn format_line_rich_mode_assistant_message() {
    let finders = RichModeFinders::new();
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":"Hello world"},"timestamp":"2026-01-15T10:30:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert_eq!(results.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "message");
    assert_eq!(parsed["role"], "assistant");
    assert_eq!(parsed["content"], "Hello world");
    assert_eq!(parsed["ts"], "2026-01-15T10:30:00Z");
}

#[test]
fn format_line_rich_mode_tool_use() {
    let finders = RichModeFinders::new();
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Read","id":"123","input":{"path":"src/main.rs"}}]}}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert_eq!(results.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "tool_use");
    assert_eq!(parsed["name"], "Read");
    assert_eq!(parsed["input"]["path"], "src/main.rs");
}

#[test]
fn format_line_rich_mode_invalid_json_skipped() {
    let finders = RichModeFinders::new();
    // Has "type" substring but isn't valid JSON — should be skipped in rich mode
    let line = r#"this is not json but has "type" in it"#;
    let result = format_line_for_mode(line, "rich", &finders);
    assert!(
        result.is_empty(),
        "Invalid JSON should be skipped in rich mode"
    );
}

#[test]
fn format_line_rich_mode_no_type_key_skipped() {
    let finders = RichModeFinders::new();
    // Valid JSON but no "type" key — skipped in rich mode
    let line = r#"{"role":"user","content":"hello"}"#;
    let result = format_line_for_mode(line, "rich", &finders);
    assert!(
        result.is_empty(),
        "Line without type key should be skipped in rich mode"
    );
}

#[test]
fn format_line_rich_mode_progress_emits_category() {
    let finders = RichModeFinders::new();
    let line = r#"{"type":"progress","data":{"type":"hook_progress","hookName":"pre-commit"}}"#;
    let result = format_line_for_mode(line, "rich", &finders);
    assert_eq!(result.len(), 1, "Progress events should emit one message");
    let parsed: serde_json::Value = serde_json::from_str(&result[0]).unwrap();
    assert_eq!(parsed["type"], "progress");
    assert_eq!(parsed["content"], "hook_progress: pre-commit");
    assert_eq!(parsed["category"], "hook");
}

#[test]
fn format_line_rich_mode_meta_skipped() {
    let finders = RichModeFinders::new();
    let line =
        r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"system prompt"}}"#;
    let result = format_line_for_mode(line, "rich", &finders);
    assert!(
        result.is_empty(),
        "Meta messages should be skipped in rich mode"
    );
}

#[test]
fn format_line_rich_mode_thinking_extracted() {
    let finders = RichModeFinders::new();
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Let me analyze this...","signature":"abc"}]},"timestamp":"2026-01-15T10:30:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert_eq!(results.len(), 1);
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "thinking");
    assert!(parsed["content"]
        .as_str()
        .unwrap()
        .contains("Let me analyze this"));
    assert_eq!(parsed["ts"], "2026-01-15T10:30:00Z");
}

#[test]
fn format_line_rich_mode_no_content_skipped() {
    let finders = RichModeFinders::new();
    // Has type but no extractable content
    let line = r#"{"type":"assistant","message":{"role":"assistant"}}"#;
    let result = format_line_for_mode(line, "rich", &finders);
    assert!(
        result.is_empty(),
        "Messages without content should be skipped"
    );
}

#[test]
fn format_line_rich_mode_multiple_content_blocks() {
    let finders = RichModeFinders::new();
    // A message with thinking + text + tool_use — all blocks should be extracted
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":"reasoning here"},{"type":"text","text":"Part 1"},{"type":"text","text":"Part 2"},{"type":"tool_use","name":"Read","id":"1","input":{"file":"a.rs"}},{"type":"tool_use","name":"Write","id":"2","input":{"file":"b.rs"}}]},"timestamp":"2026-02-16T00:00:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    // Should produce: 2 tool_use + 1 thinking + 1 text (concatenated) = 4 messages
    assert_eq!(
        results.len(),
        4,
        "Expected 4 messages, got {}: {:?}",
        results.len(),
        results
    );

    // Check tool_use messages
    let tool1: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(tool1["type"], "tool_use");
    assert_eq!(tool1["name"], "Read");
    let tool2: serde_json::Value = serde_json::from_str(&results[1]).unwrap();
    assert_eq!(tool2["type"], "tool_use");
    assert_eq!(tool2["name"], "Write");

    // Check thinking (concatenated)
    let thinking: serde_json::Value = serde_json::from_str(&results[2]).unwrap();
    assert_eq!(thinking["type"], "thinking");
    assert_eq!(thinking["content"], "reasoning here");

    // Check text (concatenated from Part 1 + Part 2)
    let text: serde_json::Value = serde_json::from_str(&results[3]).unwrap();
    assert_eq!(text["type"], "message");
    assert!(text["content"].as_str().unwrap().contains("Part 1"));
    assert!(text["content"].as_str().unwrap().contains("Part 2"));
}

// =========================================================================
// Unit tests for strip_command_tags
// =========================================================================

#[test]
fn strip_command_tags_removes_all_known_tags() {
    let input = r#"<command-name>/clear</command-name>
<command-message>clear</command-message>
<command-args></command-args>

NaN ago
<local-command-stdout></local-command-stdout>"#;
    let result = strip_command_tags(input);
    assert!(!result.contains("<command-name>"));
    assert!(!result.contains("<local-command-stdout>"));
    // After stripping all tags and trimming, only "NaN ago" should remain
    assert_eq!(result, "NaN ago");
}

#[test]
fn strip_command_tags_preserves_normal_content() {
    let input = "Here is a table:\n\n| Col1 | Col2 |\n|------|------|\n| a    | b    |";
    let result = strip_command_tags(input);
    assert_eq!(result, input);
}

#[test]
fn strip_command_tags_handles_missing_close_tag() {
    let input = "<command-name>unclosed content but normal text after";
    let result = strip_command_tags(input);
    // Should not infinite loop; returns input unchanged since no closing tag
    assert_eq!(result, input);
}

#[test]
fn strip_command_tags_empty_after_stripping_skips_string_content() {
    let finders = RichModeFinders::new();
    // Content is entirely command tags — should produce empty vec after stripping
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":"<command-name>/clear</command-name><command-args></command-args>"},"timestamp":"2026-02-16T00:00:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert!(
        results.is_empty(),
        "Messages that become empty after tag stripping should not be emitted"
    );
}

#[test]
fn strip_command_tags_empty_after_stripping_skips_text_blocks() {
    let finders = RichModeFinders::new();
    // Content is array with a text block that is entirely command tags
    let line = r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"<command-name>/clear</command-name>"}]},"timestamp":"2026-02-16T00:00:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert!(
        results.is_empty(),
        "Text blocks that become empty after tag stripping should not be emitted"
    );
}

// =========================================================================
// Unit test: queue-operation includes metadata in rich mode
// =========================================================================

#[test]
fn format_line_rich_mode_queue_operation_includes_metadata() {
    let finders = RichModeFinders::new();
    let line = r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-09T10:00:00Z","content":"fix the bug"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert_eq!(results.len(), 1, "queue-operation should emit one message");
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "system");
    assert_eq!(parsed["category"], "queue");

    // Key assertions: metadata must exist with operation, content, and type
    let metadata = &parsed["metadata"];
    assert!(
        !metadata.is_null(),
        "queue-operation must include metadata object"
    );
    assert_eq!(metadata["type"], "queue-operation");
    assert_eq!(metadata["operation"], "enqueue");
    assert_eq!(metadata["content"], "fix the bug");
}

#[test]
fn format_line_rich_mode_queue_operation_without_content() {
    let finders = RichModeFinders::new();
    let line =
        r#"{"type":"queue-operation","operation":"cancel","timestamp":"2026-03-09T10:01:00Z"}"#;
    let results = format_line_for_mode(line, "rich", &finders);
    assert_eq!(results.len(), 1, "queue-operation should emit one message");
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "system");
    assert_eq!(parsed["category"], "queue");
    assert_eq!(parsed["content"], "queue-cancel");

    let metadata = &parsed["metadata"];
    assert_eq!(metadata["type"], "queue-operation");
    assert_eq!(metadata["operation"], "cancel");
    // content should not be present when the source line has none
    assert!(
        metadata.get("content").is_none() || metadata["content"].is_null(),
        "metadata.content should be absent when source has no content"
    );
}

// =========================================================================
// Integration test: queue-operation metadata via WebSocket
// =========================================================================

#[tokio::test]
async fn test_rich_mode_queue_operation_includes_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let jsonl_path = dir.path().join("test-session.jsonl");
    {
        let mut f = std::fs::File::create(&jsonl_path).unwrap();
        writeln!(f, r#"{{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-09T10:00:00Z","content":"fix the bug"}}"#).unwrap();
    }

    let session_id = "test-queue-meta";
    let state = test_state_with_session(session_id, jsonl_path.to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;
    let mut ws = ws_connect(addr, session_id).await;

    // Send handshake (rich mode)
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"rich","scrollback":100}"#.into(),
    ))
    .await
    .unwrap();

    // Collect messages until buffer_end (with timeout via recv_text)
    let mut messages = Vec::new();
    loop {
        match recv_text(&mut ws).await {
            Some(text) => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if v.get("type").and_then(|t| t.as_str()) == Some("buffer_end") {
                        break;
                    }
                }
                messages.push(text);
            }
            None => break, // timeout — no more messages
        }
    }

    // Find the queue-operation message
    let queue_msg = messages
        .iter()
        .find(|m| m.contains("queue"))
        .expect("should have a queue message");

    let parsed: serde_json::Value = serde_json::from_str(queue_msg).unwrap();
    assert_eq!(parsed["type"], "system");
    assert_eq!(parsed["category"], "queue");

    // Key assertions: metadata must exist with operation, content, and type
    let metadata = &parsed["metadata"];
    assert_eq!(metadata["type"], "queue-operation");
    assert_eq!(metadata["operation"], "enqueue");
    assert_eq!(metadata["content"], "fix the bug");

    server_handle.abort();
}

// =========================================================================
// Test: ws_block_mode_scrollback
// =========================================================================

#[tokio::test]
async fn ws_block_mode_scrollback() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    writeln!(
        tmp.as_file(),
        r#"{{"type":"user","uuid":"u-1","message":{{"content":[{{"type":"text","text":"hello"}}]}},"timestamp":"2026-03-21T01:00:00.000Z"}}"#
    )
    .unwrap();

    let state = test_state_with_session("ws-block-test", tmp.path().to_str().unwrap()).await;
    let (addr, server_handle) = start_test_server(state).await;
    let mut ws = ws_connect(addr, "ws-block-test").await;

    // Send handshake with block mode
    ws.send(tungstenite::Message::Text(
        r#"{"mode":"block","scrollback":50}"#.into(),
    ))
    .await
    .unwrap();

    // Collect scrollback messages until we see buffer_end or timeout
    let mut received: Vec<serde_json::Value> = Vec::new();
    let timeout_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while let Some(text) = recv_text(&mut ws).await {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                let msg_type = json.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if msg_type == "buffer_end" {
                    break;
                }
                received.push(json);
            }
        }
    })
    .await;
    assert!(
        timeout_result.is_ok(),
        "Should receive buffer_end within timeout"
    );

    // Should have received at least one block
    assert!(
        !received.is_empty(),
        "Should receive scrollback blocks in block mode"
    );

    // First received block should have a type discriminator
    let first = &received[0];
    assert!(
        first.get("type").is_some(),
        "Block should have 'type' discriminator"
    );

    ws.close(None).await.ok();
    server_handle.abort();
}

// =========================================================================
// Test: format_line_block_mode_produces_conversation_blocks
// =========================================================================

#[test]
fn format_line_block_mode_produces_conversation_blocks() {
    let finders = RichModeFinders::new();
    // A user message line
    let line = r#"{"type":"user","uuid":"u-1","message":{"content":[{"type":"text","text":"hello world"}]},"timestamp":"2026-03-21T01:00:00.000Z"}"#;
    let results = format_line_for_mode(line, "block", &finders);
    assert!(!results.is_empty(), "Block mode should produce output");
    let parsed: serde_json::Value = serde_json::from_str(&results[0]).unwrap();
    assert_eq!(parsed["type"], "user", "Should produce a UserBlock");
    assert_eq!(parsed["text"], "hello world");
}
