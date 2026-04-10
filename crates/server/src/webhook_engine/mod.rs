//! Webhook notification engine.
//!
//! Subscribes to `broadcast::Sender<SessionEvent>`, formats events,
//! and delivers HMAC-signed HTTP POST requests to configured endpoints.

pub mod config;
pub mod debounce;
pub mod delivery;
pub mod formatters;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::{broadcast, watch};

use claude_view_server_live_state::event::SessionEvent;
use config::{WebhookEventType, WebhookSecrets};
use debounce::Debouncer;

/// Spawn the webhook engine as a background tokio task.
///
/// Subscribes to the live session broadcast channel and delivers webhook
/// notifications for configured endpoints. Re-reads config from disk on
/// every event for hot-reload support.
pub fn spawn_engine(
    live_tx: &broadcast::Sender<SessionEvent>,
    shutdown: watch::Receiver<bool>,
    config_path: PathBuf,
    secrets_path: PathBuf,
    base_url: Option<String>,
) -> tokio::task::JoinHandle<()> {
    let mut rx = live_tx.subscribe();
    let mut shutdown = shutdown;

    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let mut debouncer = Debouncer::new(Duration::from_secs(10));
        let mut error_tracker: HashMap<String, bool> = HashMap::new();

        tracing::info!("Webhook engine started");

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(session_event) => {
                            handle_event(
                                &session_event,
                                &client,
                                &config_path,
                                &secrets_path,
                                base_url.as_deref(),
                                &mut debouncer,
                                &mut error_tracker,
                            ).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!(skipped = n, "Webhook engine lagged, skipping events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            tracing::info!("Webhook engine: broadcast channel closed, shutting down");
                            break;
                        }
                    }
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("Webhook engine: shutdown signal received");
                        break;
                    }
                }
            }
        }

        tracing::info!("Webhook engine stopped");
    })
}

/// Map a `SessionEvent` to webhook event types, then deliver to matching webhooks.
async fn handle_event(
    event: &SessionEvent,
    client: &reqwest::Client,
    config_path: &PathBuf,
    secrets_path: &PathBuf,
    base_url: Option<&str>,
    debouncer: &mut Debouncer,
    error_tracker: &mut HashMap<String, bool>,
) {
    // Map event to webhook event type(s) + extract session reference.
    let (event_types, session) = match event {
        SessionEvent::SessionDiscovered { session } => {
            error_tracker.insert(session.id.clone(), false);
            (vec![WebhookEventType::SessionStarted], session)
        }
        SessionEvent::SessionClosed { session } => {
            debouncer.remove_session(&session.id);
            error_tracker.remove(&session.id);
            (vec![WebhookEventType::SessionEnded], session)
        }
        SessionEvent::SessionUpdated { session } => {
            let mut types = Vec::new();

            // Error detection: emit session.error on first error transition.
            if session.hook.last_error.is_some() {
                let had_error = error_tracker.entry(session.id.clone()).or_insert(false);
                if !*had_error {
                    *had_error = true;
                    types.push(WebhookEventType::SessionError);
                }
            }

            types.push(WebhookEventType::SessionUpdated);
            (types, session)
        }
        SessionEvent::SessionCompleted { .. }
        | SessionEvent::Summary { .. }
        | SessionEvent::CliSessionCreated { .. }
        | SessionEvent::CliSessionUpdated { .. }
        | SessionEvent::CliSessionRemoved { .. } => return,
    };

    // Load config (file-backed, re-read each time for hot reload).
    let config = config::load_config(config_path);
    if config.webhooks.is_empty() {
        return;
    }
    let secrets = config::load_secrets(secrets_path);

    deliver_to_webhooks(
        &event_types,
        session,
        &config,
        &secrets,
        client,
        base_url,
        debouncer,
    );
}

/// For each event type, deliver to matching enabled webhooks.
fn deliver_to_webhooks(
    event_types: &[WebhookEventType],
    session: &claude_view_server_live_state::core::LiveSession,
    config: &config::NotificationsConfig,
    secrets: &WebhookSecrets,
    client: &reqwest::Client,
    base_url: Option<&str>,
    debouncer: &mut Debouncer,
) {
    for event_type in event_types {
        for webhook in &config.webhooks {
            if !webhook.enabled || !webhook.events.contains(event_type) {
                continue;
            }

            // Debounce session.updated events.
            if *event_type == WebhookEventType::SessionUpdated
                && !debouncer.should_send(&webhook.id, &session.id)
            {
                continue;
            }

            // Get signing secret.
            let secret = match secrets.secrets.get(&webhook.id) {
                Some(s) => s,
                None => {
                    tracing::warn!(webhook_id = %webhook.id, "No signing secret found, skipping");
                    continue;
                }
            };

            // Build and format payload.
            let payload = formatters::build_payload(event_type, session, base_url);
            let formatted = formatters::format_payload(&payload, &webhook.format);
            let body = serde_json::to_string(&formatted).unwrap_or_default();

            // Deliver asynchronously (don't block the event loop for retries).
            let client = client.clone();
            let url = webhook.url.clone();
            let webhook_id = payload.id.clone();
            let secret = secret.clone();
            tokio::spawn(async move {
                let result = delivery::deliver(&client, &url, &webhook_id, body, &secret, 3).await;
                if result.success {
                    tracing::debug!(webhook_id = %webhook_id, "Webhook delivered successfully");
                } else {
                    tracing::warn!(
                        webhook_id = %webhook_id,
                        error = ?result.error,
                        attempts = result.attempts,
                        "Webhook delivery failed"
                    );
                }
            });
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_server_live_state::core::test_live_session;
    use claude_view_server_live_state::event::SessionEvent;
    use tempfile::TempDir;

    #[tokio::test]
    async fn engine_starts_and_stops_cleanly() {
        let (tx, _) = broadcast::channel(16);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let tmp = TempDir::new().unwrap();

        let handle = spawn_engine(
            &tx,
            shutdown_rx,
            tmp.path().join("notifications.json"),
            tmp.path().join("secrets.json"),
            None,
        );

        // Signal shutdown.
        shutdown_tx.send(true).unwrap();
        // Should complete within a reasonable time.
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn engine_handles_discovered_event() {
        let (tx, _) = broadcast::channel(16);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let tmp = TempDir::new().unwrap();

        let handle = spawn_engine(
            &tx,
            shutdown_rx,
            tmp.path().join("notifications.json"),
            tmp.path().join("secrets.json"),
            None,
        );

        // Send a session event (no webhooks configured, so nothing happens, but no panic).
        let session = test_live_session("test-sess");
        tx.send(SessionEvent::SessionDiscovered { session })
            .unwrap();

        // Give it a moment to process.
        tokio::time::sleep(Duration::from_millis(50)).await;

        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn engine_skips_completed_and_summary() {
        let (tx, _) = broadcast::channel(16);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let tmp = TempDir::new().unwrap();

        let handle = spawn_engine(
            &tx,
            shutdown_rx,
            tmp.path().join("notifications.json"),
            tmp.path().join("secrets.json"),
            None,
        );

        // These should be skipped without error.
        tx.send(SessionEvent::SessionCompleted {
            session_id: "old".into(),
        })
        .unwrap();
        tx.send(SessionEvent::Summary {
            needs_you_count: 0,
            autonomous_count: 0,
            total_cost_today_usd: 0.0,
            total_tokens_today: 0,
        })
        .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn engine_delivers_to_configured_webhook() {
        // Set up a mock HTTP server.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let received_clone = received.clone();

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let received = received_clone.clone();
                let io = hyper_util::rt::TokioIo::new(stream);
                tokio::spawn(async move {
                    hyper::server::conn::http1::Builder::new()
                        .serve_connection(
                            io,
                            hyper::service::service_fn(move |req| {
                                let received = received.clone();
                                async move {
                                    let body = http_body_util::BodyExt::collect(req.into_body())
                                        .await
                                        .unwrap()
                                        .to_bytes();
                                    received
                                        .lock()
                                        .unwrap()
                                        .push(String::from_utf8(body.to_vec()).unwrap());
                                    Ok::<_, std::convert::Infallible>(hyper::Response::new(
                                        http_body_util::Full::new(hyper::body::Bytes::from("ok")),
                                    ))
                                }
                            }),
                        )
                        .await
                        .ok();
                });
            }
        });

        // Configure a webhook pointing to our mock server.
        let tmp = TempDir::new().unwrap();
        let wh_config = config::NotificationsConfig {
            base_url: None,
            webhooks: vec![config::WebhookConfig {
                id: "wh_test".into(),
                name: "test".into(),
                url: format!("http://{addr}/webhook"),
                format: config::WebhookFormat::Raw,
                events: vec![config::WebhookEventType::SessionStarted],
                enabled: true,
                created_at: "2026-04-10".into(),
            }],
        };
        let secret = config::generate_signing_secret();
        let mut secrets = config::WebhookSecrets::default();
        secrets.secrets.insert("wh_test".into(), secret);

        let config_path = tmp.path().join("notifications.json");
        let secrets_path = tmp.path().join("secrets.json");
        config::save_config(&wh_config, &config_path).unwrap();
        config::save_secrets(&secrets, &secrets_path).unwrap();

        let (tx, _) = broadcast::channel(16);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let handle = spawn_engine(&tx, shutdown_rx, config_path, secrets_path, None);

        // Send a session event.
        let session = test_live_session("integration-test");
        tx.send(SessionEvent::SessionDiscovered { session })
            .unwrap();

        // Wait for delivery.
        tokio::time::sleep(Duration::from_millis(200)).await;

        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap()
            .unwrap();

        // Verify the mock server received the webhook.
        let payloads = received.lock().unwrap();
        assert_eq!(
            payloads.len(),
            1,
            "Expected 1 delivery, got {}",
            payloads.len()
        );
        let payload: serde_json::Value = serde_json::from_str(&payloads[0]).unwrap();
        assert_eq!(payload["type"], "session.started");
        assert_eq!(payload["data"]["sessionId"], "integration-test");
    }
}
