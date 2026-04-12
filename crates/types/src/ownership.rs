// crates/types/src/ownership.rs
//
// Session ownership as independent facts + pending interaction metadata.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InteractionVariant;

// ── Session Ownership ─────────────────────────────────────────────

/// Session ownership as independent facts. Both tmux and sdk can be
/// simultaneously true (e.g. SDK controls a session spawned in our tmux).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SessionOwnership {
    /// Set when session runs inside a claude-view managed tmux pane.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux: Option<TmuxBinding>,
    /// Set when Agent SDK has bound control to this session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sdk: Option<SdkBinding>,
    /// Origin category: "terminal", "ide", "agent_sdk".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// How the session was started: "cli", "claude-vscode", etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
}

/// Tmux pane binding — session runs inside a claude-view managed tmux pane.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct TmuxBinding {
    pub cli_session_id: String,
}

/// SDK control binding — Agent SDK has taken control of this session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SdkBinding {
    pub control_id: String,
}

// ── Pending Interaction Meta ──────────────────────────────────────

/// Metadata for a pending user interaction (permission prompt, question, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PendingInteractionMeta {
    pub variant: InteractionVariant,
    pub request_id: String,
    pub preview: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SessionOwnership (struct) ─────────────────────────────────

    #[test]
    fn tmux_only_serializes_correctly() {
        let val = SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-abc".into(),
            }),
            sdk: None,
            source: Some("terminal".into()),
            entrypoint: Some("cli".into()),
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["tmux"]["cliSessionId"], "cv-abc");
        assert!(json.get("sdk").is_none()); // skip_serializing_if
        assert_eq!(json["source"], "terminal");
        assert_eq!(json["entrypoint"], "cli");
        assert!(json.get("tier").is_none());
    }

    #[test]
    fn sdk_only_serializes_correctly() {
        let val = SessionOwnership {
            tmux: None,
            sdk: Some(SdkBinding {
                control_id: "ctl-123".into(),
            }),
            source: Some("agent_sdk".into()),
            entrypoint: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert!(json.get("tmux").is_none());
        assert_eq!(json["sdk"]["controlId"], "ctl-123");
    }

    #[test]
    fn both_tmux_and_sdk_coexist() {
        let val = SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-99".into(),
            }),
            sdk: Some(SdkBinding {
                control_id: "ctl-77".into(),
            }),
            source: None,
            entrypoint: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["tmux"]["cliSessionId"], "cv-99");
        assert_eq!(json["sdk"]["controlId"], "ctl-77");
    }

    #[test]
    fn neither_tmux_nor_sdk() {
        let val = SessionOwnership {
            tmux: None,
            sdk: None,
            source: Some("ide".into()),
            entrypoint: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert!(json.get("tmux").is_none());
        assert!(json.get("sdk").is_none());
        assert_eq!(json["source"], "ide");
    }

    #[test]
    fn round_trip_deserialization() {
        let original = SessionOwnership {
            tmux: Some(TmuxBinding {
                cli_session_id: "cv-1".into(),
            }),
            sdk: None,
            source: Some("terminal".into()),
            entrypoint: Some("cli".into()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let back: SessionOwnership = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn default_is_empty() {
        let val = SessionOwnership::default();
        assert!(val.tmux.is_none());
        assert!(val.sdk.is_none());
        assert!(val.source.is_none());
        assert!(val.entrypoint.is_none());
    }

    // ── PendingInteractionMeta ────────────────────────────────────

    #[test]
    fn pending_interaction_meta_round_trips_permission() {
        let meta = PendingInteractionMeta {
            variant: InteractionVariant::Permission,
            request_id: "req-001".into(),
            preview: "Allow file write?".into(),
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["variant"], "permission");
        assert_eq!(json["requestId"], "req-001");
        assert_eq!(json["preview"], "Allow file write?");
        // Must NOT have snake_case
        assert!(json.get("request_id").is_none());

        // Round-trip
        let back: PendingInteractionMeta = serde_json::from_value(json).unwrap();
        assert_eq!(back.request_id, "req-001");
    }

    #[test]
    fn pending_interaction_meta_round_trips_all_variants() {
        for variant in [
            InteractionVariant::Permission,
            InteractionVariant::Question,
            InteractionVariant::Plan,
            InteractionVariant::Elicitation,
        ] {
            let meta = PendingInteractionMeta {
                variant,
                request_id: "r".into(),
                preview: "p".into(),
            };
            let json = serde_json::to_string(&meta).unwrap();
            let back: PendingInteractionMeta = serde_json::from_str(&json).unwrap();
            assert_eq!(
                format!("{:?}", back.variant),
                format!("{:?}", variant),
                "round-trip failed for {:?}",
                variant
            );
        }
    }
}
