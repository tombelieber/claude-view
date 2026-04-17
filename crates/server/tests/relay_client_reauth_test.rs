//! Smoke test for the relay client's JWT attachment + register_device
//! round-trip. Uses an in-process tokio-tungstenite server.

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;

#[tokio::test]
async fn attach_token_appends_query_param_when_none() {
    let url = claude_view_server::live::relay_client::__test_attach_token("wss://host/ws", "abc");
    assert_eq!(url, "wss://host/ws?token=abc");
}

#[tokio::test]
async fn attach_token_appends_query_param_when_existing() {
    let url = claude_view_server::live::relay_client::__test_attach_token(
        "wss://host/ws?region=nrt",
        "abc",
    );
    assert_eq!(url, "wss://host/ws?region=nrt&token=abc");
}

#[tokio::test]
async fn minimal_echo_server_accepts_jwt_query_param() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut ws = accept_async(stream).await.unwrap();
        // Receive register_device
        let msg = ws.next().await.unwrap().unwrap();
        assert!(msg.to_text().unwrap().contains("register_device"));
        // Reply auth_ok
        ws.send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"type":"auth_ok","user_id":"u","device_count":1}"#.into(),
        ))
        .await
        .unwrap();
    });

    let url = format!("ws://{addr}/?token=test");
    let (mut client, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    client
        .send(tokio_tungstenite::tungstenite::Message::Text(
            r#"{"type":"register_device","device_id":"mac-test","platform":"mac","display_name":"x"}"#
                .into(),
        ))
        .await
        .unwrap();
    let reply = client.next().await.unwrap().unwrap();
    assert!(reply.to_text().unwrap().contains("auth_ok"));
    server.await.unwrap();
}
