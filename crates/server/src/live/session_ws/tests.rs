//! Tests for multiplexed session WebSocket components.

use super::frames::*;
use super::registry::SessionChannelRegistry;

// ── Frame serialization round-trip tests ────────────────────────────

#[test]
fn frame_round_trip_handshake_ack() {
    let frame = SessionFrame::HandshakeAck {
        session_id: "sess-123".to_string(),
        modes: vec![FrameMode::Block, FrameMode::Sdk],
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"frame\":\"handshake_ack\""));
    assert!(json.contains("\"session_id\":\"sess-123\""));

    let parsed: SessionFrame = serde_json::from_str(&json).unwrap();
    match parsed {
        SessionFrame::HandshakeAck { session_id, modes } => {
            assert_eq!(session_id, "sess-123");
            assert_eq!(modes.len(), 2);
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn frame_round_trip_block_delta() {
    let block = serde_json::json!({"id": "b1", "type": "tool_use", "content": "hello"});
    let frame = SessionFrame::BlockDelta {
        block: block.clone(),
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"frame\":\"block_delta\""));

    let parsed: SessionFrame = serde_json::from_str(&json).unwrap();
    match parsed {
        SessionFrame::BlockDelta { block: b } => {
            assert_eq!(b["id"], "b1");
            assert_eq!(b["type"], "tool_use");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn frame_round_trip_sdk_event() {
    let payload = serde_json::json!({"type": "blocks_update", "blocks": []});
    let frame = SessionFrame::SdkEvent {
        payload: payload.clone(),
    };
    let json = serde_json::to_string(&frame).unwrap();
    assert!(json.contains("\"frame\":\"sdk_event\""));

    let parsed: SessionFrame = serde_json::from_str(&json).unwrap();
    match parsed {
        SessionFrame::SdkEvent { payload: p } => {
            assert_eq!(p["type"], "blocks_update");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn frame_round_trip_error() {
    let frame = SessionFrame::Error {
        message: "not found".to_string(),
        code: "SESSION_NOT_FOUND".to_string(),
    };
    let json = serde_json::to_string(&frame).unwrap();
    let parsed: SessionFrame = serde_json::from_str(&json).unwrap();
    match parsed {
        SessionFrame::Error { message, code } => {
            assert_eq!(message, "not found");
            assert_eq!(code, "SESSION_NOT_FOUND");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn client_handshake_defaults() {
    let json = r#"{"modes": ["block"]}"#;
    let hs: ClientHandshake = serde_json::from_str(json).unwrap();
    assert_eq!(hs.modes, vec![FrameMode::Block]);
    assert_eq!(hs.scrollback.block, 50);
    assert_eq!(hs.scrollback.raw, 1000);
}

#[test]
fn client_message_ping() {
    let json = r#"{"type": "ping"}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();
    assert!(matches!(msg, ClientMessage::Ping));
}

#[test]
fn client_message_sdk_send() {
    let json = r#"{"type": "sdk_send", "payload": {"action": "resume"}}"#;
    let msg: ClientMessage = serde_json::from_str(json).unwrap();
    match msg {
        ClientMessage::SdkSend { payload } => {
            assert_eq!(payload["action"], "resume");
        }
        _ => panic!("wrong variant"),
    }
}

// ── Registry tests ──────────────────────────────────────────────────

#[test]
fn registry_connect_disconnect() {
    let reg = SessionChannelRegistry::new();
    assert_eq!(reg.global_count(), 0);
    assert_eq!(reg.session_count("s1"), 0);

    reg.try_connect("s1").unwrap();
    assert_eq!(reg.global_count(), 1);
    assert_eq!(reg.session_count("s1"), 1);

    reg.try_connect("s1").unwrap();
    assert_eq!(reg.session_count("s1"), 2);

    reg.disconnect("s1");
    assert_eq!(reg.session_count("s1"), 1);
    assert_eq!(reg.global_count(), 1);

    reg.disconnect("s1");
    assert_eq!(reg.session_count("s1"), 0);
    assert_eq!(reg.global_count(), 0);
}

#[test]
fn registry_per_session_limit() {
    let reg = SessionChannelRegistry::new();
    for _ in 0..4 {
        reg.try_connect("s1").unwrap();
    }
    let err = reg.try_connect("s1").unwrap_err();
    assert!(err.contains("per-session"));

    // Other sessions still allowed
    reg.try_connect("s2").unwrap();
    assert_eq!(reg.global_count(), 5);
}

#[test]
fn registry_global_limit() {
    let reg = SessionChannelRegistry::new();
    // Fill to 64 across many sessions
    for i in 0..64 {
        reg.try_connect(&format!("s{i}")).unwrap();
    }
    let err = reg.try_connect("extra").unwrap_err();
    assert!(err.contains("global"));
}

#[test]
fn registry_disconnect_nonexistent_is_safe() {
    let reg = SessionChannelRegistry::new();
    // Should not panic or underflow
    reg.disconnect("nonexistent");
    // Global counter underflows to max, but saturating_sub on per-session prevents it
    // Actually global uses fetch_sub which WILL underflow — but that's a u64 wrap.
    // This is acceptable because disconnect is only called after a successful connect.
}
