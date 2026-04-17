//! Integration test: two clients in the same Supabase user_id exchange
//! messages via the stateless relay; a third client in a different
//! user_id does not see them.
//!
//! Uses a MockSupabaseClient so we don't need a real Supabase project.
//! Uses an in-process axum test server and tokio-tungstenite clients.

use std::sync::Arc;
use std::time::Duration;

use claude_view_relay::auth::SupabaseAuth;
use claude_view_relay::device_cache::DeviceCache;
use claude_view_relay::rate_limit::RateLimiter;
use claude_view_relay::state::RelayState;
use claude_view_relay::supabase::{DeviceRow, MockSupabaseClient, SupabaseClient};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// Build a fake but parseable JWT for tests — no signature, `SupabaseAuth::Mock`
/// accepts anything whose payload base64-decodes to `{"sub": "..."}`.
fn fake_jwt(user_id: &str) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let body = URL_SAFE_NO_PAD.encode(format!(r#"{{"sub":"{user_id}","exp":9999999999}}"#));
    let sig = URL_SAFE_NO_PAD.encode("fake-sig");
    format!("{header}.{body}.{sig}")
}

async fn spawn_test_relay(
    mock_sb: Arc<MockSupabaseClient>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let device_cache = Arc::new(DeviceCache::new(
        mock_sb as Arc<dyn SupabaseClient>,
        Duration::from_secs(60),
    ));

    let state = RelayState {
        connections: Arc::new(DashMap::new()),
        supabase_auth: Some(Arc::new(SupabaseAuth::mock_for_test())),
        http: reqwest::Client::new(),
        device_cache,
        onesignal_app_id: None,
        onesignal_rest_api_key: None,
        onesignal_http: None,
        posthog_api_key: None,
        posthog_http: None,
        ws_rate_limiter: Arc::new(RateLimiter::new(1000.0, 1000.0)),
        push_rate_limiter: Arc::new(RateLimiter::new(1000.0, 1000.0)),
    };

    let app = claude_view_relay::app(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to become ready.
    tokio::time::sleep(Duration::from_millis(50)).await;

    (addr, handle)
}

#[tokio::test]
async fn two_clients_same_user_exchange_messages() {
    let mock = Arc::new(MockSupabaseClient::default());
    mock.insert(DeviceRow {
        device_id: "mac-1111111111111111".to_string(),
        user_id: "user-1".to_string(),
        platform: "mac".to_string(),
        revoked_at: None,
    });
    mock.insert(DeviceRow {
        device_id: "ios-2222222222222222".to_string(),
        user_id: "user-1".to_string(),
        platform: "ios".to_string(),
        revoked_at: None,
    });

    let (addr, _handle) = spawn_test_relay(mock).await;

    // Connect client A (Mac).
    let url_a = format!("ws://{addr}/ws?token={}", fake_jwt("user-1"));
    let (mut a, _) = tokio_tungstenite::connect_async(&url_a).await.unwrap();
    a.send(Message::Text(
        json!({
            "type": "register_device",
            "device_id": "mac-1111111111111111",
            "platform": "mac",
            "display_name": "Test Mac"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let auth_ok_a = a.next().await.unwrap().unwrap();
    let auth_ok_a_text = match auth_ok_a {
        Message::Text(t) => t.to_string(),
        _ => panic!("expected text auth_ok"),
    };
    assert!(
        auth_ok_a_text.contains("auth_ok"),
        "A auth_ok: {auth_ok_a_text}"
    );

    // Connect client B (iOS).
    let url_b = format!("ws://{addr}/ws?token={}", fake_jwt("user-1"));
    let (mut b, _) = tokio_tungstenite::connect_async(&url_b).await.unwrap();
    b.send(Message::Text(
        json!({
            "type": "register_device",
            "device_id": "ios-2222222222222222",
            "platform": "ios",
            "display_name": "Test iPhone"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let auth_ok_b = b.next().await.unwrap().unwrap();
    let auth_ok_b_text = match auth_ok_b {
        Message::Text(t) => t.to_string(),
        _ => panic!("expected text auth_ok"),
    };
    assert!(
        auth_ok_b_text.contains("auth_ok"),
        "B auth_ok: {auth_ok_b_text}"
    );

    // Client A sends a session_update targeting B.
    a.send(Message::Text(
        json!({
            "type": "session_update",
            "to_device_id": "ios-2222222222222222",
            "payload_b64": "aGVsbG8="
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();

    // B should receive it.
    let received = tokio::time::timeout(Duration::from_secs(2), b.next())
        .await
        .expect("B didn't receive message within timeout")
        .unwrap()
        .unwrap();
    let text = match received {
        Message::Text(t) => t.to_string(),
        _ => panic!("expected text from B"),
    };
    assert!(text.contains("session_update"), "got: {text}");
    assert!(text.contains("aGVsbG8="), "got: {text}");
    assert!(
        text.contains("mac-1111111111111111"),
        "from_device_id missing: {text}"
    );
}

#[tokio::test]
async fn different_users_do_not_see_each_others_messages() {
    let mock = Arc::new(MockSupabaseClient::default());
    mock.insert(DeviceRow {
        device_id: "mac-aaaaaaaaaaaaaaaa".to_string(),
        user_id: "user-a".to_string(),
        platform: "mac".to_string(),
        revoked_at: None,
    });
    mock.insert(DeviceRow {
        device_id: "ios-bbbbbbbbbbbbbbbb".to_string(),
        user_id: "user-b".to_string(),
        platform: "ios".to_string(),
        revoked_at: None,
    });

    let (addr, _handle) = spawn_test_relay(mock).await;

    // Client A — user-a's Mac.
    let (mut a, _) =
        tokio_tungstenite::connect_async(&format!("ws://{addr}/ws?token={}", fake_jwt("user-a")))
            .await
            .unwrap();
    a.send(Message::Text(
        json!({
            "type": "register_device",
            "device_id": "mac-aaaaaaaaaaaaaaaa",
            "platform": "mac",
            "display_name": "A's Mac"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let _ = a.next().await.unwrap().unwrap(); // auth_ok

    // Client B — user-b's iPhone.
    let (mut b, _) =
        tokio_tungstenite::connect_async(&format!("ws://{addr}/ws?token={}", fake_jwt("user-b")))
            .await
            .unwrap();
    b.send(Message::Text(
        json!({
            "type": "register_device",
            "device_id": "ios-bbbbbbbbbbbbbbbb",
            "platform": "ios",
            "display_name": "B's iPhone"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();
    let _ = b.next().await.unwrap().unwrap(); // auth_ok

    // A sends a message addressed to B's device_id (should be dropped — wrong user_id).
    a.send(Message::Text(
        json!({
            "type": "session_update",
            "to_device_id": "ios-bbbbbbbbbbbbbbbb",
            "payload_b64": "c2hvdWxkLWJsb2Nr"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();

    // B should NOT receive anything within 500ms.
    let result = tokio::time::timeout(Duration::from_millis(500), b.next()).await;
    assert!(
        result.is_err(),
        "B received a message from A despite being a different user: {result:?}"
    );
}

#[tokio::test]
async fn unregistered_device_id_gets_auth_error() {
    let mock = Arc::new(MockSupabaseClient::default());
    // Intentionally do NOT insert this device.

    let (addr, _handle) = spawn_test_relay(mock).await;

    let (mut ws, _) =
        tokio_tungstenite::connect_async(&format!("ws://{addr}/ws?token={}", fake_jwt("user-x")))
            .await
            .unwrap();
    ws.send(Message::Text(
        json!({
            "type": "register_device",
            "device_id": "mac-ffffffffffffffff",
            "platform": "mac",
            "display_name": "Ghost Mac"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();

    let received = ws.next().await.unwrap().unwrap();
    let text = match received {
        Message::Text(t) => t.to_string(),
        _ => panic!("expected text"),
    };
    assert!(text.contains("auth_error"), "got: {text}");
    assert!(text.contains("DEVICE_NOT_OWNED"), "got: {text}");
}
