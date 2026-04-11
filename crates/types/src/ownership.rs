// crates/types/src/ownership.rs
//
// Session ownership tiers and pending interaction metadata.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InteractionVariant;

// ── Session Ownership ─────────────────────────────────────────────

/// Discriminated union for session ownership, resolved server-side.
/// Three tiers: SDK-controlled, tmux-attached, or passively observed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum SessionOwnership {
    #[serde(rename_all = "camelCase")]
    Sdk {
        control_id: String,
        source: Option<String>,
        entrypoint: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Tmux {
        cli_session_id: String,
        source: Option<String>,
        entrypoint: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Observed {
        source: Option<String>,
        entrypoint: Option<String>,
    },
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

    // ── SessionOwnership::Sdk ─────────────────────────────────────

    #[test]
    fn sdk_serializes_with_tier_tag_and_camel_case() {
        let val = SessionOwnership::Sdk {
            control_id: "ctl-123".into(),
            source: Some("api".into()),
            entrypoint: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["tier"], "sdk");
        assert_eq!(json["controlId"], "ctl-123");
        assert_eq!(json["source"], "api");
        assert!(json.get("entrypoint").unwrap().is_null());
        // Must NOT have snake_case keys
        assert!(json.get("control_id").is_none());
    }

    #[test]
    fn sdk_deserializes_from_json() {
        let json = r#"{"tier":"sdk","controlId":"ctl-456","source":null,"entrypoint":"main"}"#;
        let val: SessionOwnership = serde_json::from_str(json).unwrap();
        match val {
            SessionOwnership::Sdk {
                control_id,
                source,
                entrypoint,
            } => {
                assert_eq!(control_id, "ctl-456");
                assert!(source.is_none());
                assert_eq!(entrypoint.as_deref(), Some("main"));
            }
            _ => panic!("expected Sdk variant"),
        }
    }

    // ── SessionOwnership::Tmux ────────────────────────────────────

    #[test]
    fn tmux_serializes_with_tier_tag_and_camel_case() {
        let val = SessionOwnership::Tmux {
            cli_session_id: "sess-789".into(),
            source: None,
            entrypoint: Some("/bin/bash".into()),
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["tier"], "tmux");
        assert_eq!(json["cliSessionId"], "sess-789");
        assert!(json.get("cli_session_id").is_none());
    }

    #[test]
    fn tmux_deserializes_from_json() {
        let json = r#"{"tier":"tmux","cliSessionId":"sess-abc","source":"hook","entrypoint":null}"#;
        let val: SessionOwnership = serde_json::from_str(json).unwrap();
        match val {
            SessionOwnership::Tmux {
                cli_session_id,
                source,
                entrypoint,
            } => {
                assert_eq!(cli_session_id, "sess-abc");
                assert_eq!(source.as_deref(), Some("hook"));
                assert!(entrypoint.is_none());
            }
            _ => panic!("expected Tmux variant"),
        }
    }

    // ── SessionOwnership::Observed ────────────────────────────────

    #[test]
    fn observed_serializes_with_tier_tag() {
        let val = SessionOwnership::Observed {
            source: Some("fs-watch".into()),
            entrypoint: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["tier"], "observed");
        assert_eq!(json["source"], "fs-watch");
    }

    #[test]
    fn observed_deserializes_from_json() {
        let json = r#"{"tier":"observed","source":null,"entrypoint":null}"#;
        let val: SessionOwnership = serde_json::from_str(json).unwrap();
        match val {
            SessionOwnership::Observed { source, entrypoint } => {
                assert!(source.is_none());
                assert!(entrypoint.is_none());
            }
            _ => panic!("expected Observed variant"),
        }
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
