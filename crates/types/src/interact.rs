// crates/types/src/interact.rs
//
// Wire format for POST /api/sessions/{id}/interact.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Request body for the interact endpoint. Tagged by `variant`.
#[derive(Debug, Clone, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[cfg_attr(feature = "codegen", ts(export))]
#[serde(tag = "variant", rename_all = "snake_case")]
pub enum InteractRequest {
    #[serde(rename_all = "camelCase")]
    Permission {
        request_id: String,
        allowed: bool,
        #[serde(default)]
        updated_permissions: Option<Vec<serde_json::Value>>,
    },
    #[serde(rename_all = "camelCase")]
    Question {
        request_id: String,
        answers: HashMap<String, String>,
    },
    #[serde(rename_all = "camelCase")]
    Plan {
        request_id: String,
        approved: bool,
        feedback: Option<String>,
        bypass_permissions: Option<bool>,
    },
    #[serde(rename_all = "camelCase")]
    Elicitation {
        request_id: String,
        response: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Permission ────────────────────────────────────────────────

    #[test]
    fn permission_deserializes_minimal() {
        let json = r#"{"variant":"permission","requestId":"abc","allowed":true}"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Permission {
                request_id,
                allowed,
                updated_permissions,
            } => {
                assert_eq!(request_id, "abc");
                assert!(allowed);
                assert!(updated_permissions.is_none());
            }
            _ => panic!("expected Permission"),
        }
    }

    #[test]
    fn permission_deserializes_with_updated_permissions() {
        let json = r#"{
            "variant": "permission",
            "requestId": "def",
            "allowed": false,
            "updatedPermissions": [{"tool": "bash", "allow": true}]
        }"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Permission {
                updated_permissions,
                ..
            } => {
                let perms = updated_permissions.unwrap();
                assert_eq!(perms.len(), 1);
                assert_eq!(perms[0]["tool"], "bash");
            }
            _ => panic!("expected Permission"),
        }
    }

    #[test]
    fn permission_serializes_camel_case() {
        let val = InteractRequest::Permission {
            request_id: "r1".into(),
            allowed: true,
            updated_permissions: None,
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["variant"], "permission");
        assert_eq!(json["requestId"], "r1");
        assert!(json.get("request_id").is_none());
    }

    // ── Question ──────────────────────────────────────────────────

    #[test]
    fn question_deserializes_with_answers() {
        let json = r#"{
            "variant": "question",
            "requestId": "q1",
            "answers": {"name": "Alice", "role": "dev"}
        }"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Question {
                request_id,
                answers,
            } => {
                assert_eq!(request_id, "q1");
                assert_eq!(answers["name"], "Alice");
                assert_eq!(answers["role"], "dev");
            }
            _ => panic!("expected Question"),
        }
    }

    // ── Plan ──────────────────────────────────────────────────────

    #[test]
    fn plan_deserializes_with_optional_fields() {
        let json = r#"{
            "variant": "plan",
            "requestId": "p1",
            "approved": true,
            "feedback": "Looks good",
            "bypassPermissions": true
        }"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Plan {
                request_id,
                approved,
                feedback,
                bypass_permissions,
            } => {
                assert_eq!(request_id, "p1");
                assert!(approved);
                assert_eq!(feedback.as_deref(), Some("Looks good"));
                assert_eq!(bypass_permissions, Some(true));
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn plan_deserializes_without_optional_fields() {
        let json = r#"{"variant":"plan","requestId":"p2","approved":false}"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Plan {
                feedback,
                bypass_permissions,
                ..
            } => {
                assert!(feedback.is_none());
                assert!(bypass_permissions.is_none());
            }
            _ => panic!("expected Plan"),
        }
    }

    #[test]
    fn plan_serializes_camel_case() {
        let val = InteractRequest::Plan {
            request_id: "p3".into(),
            approved: true,
            feedback: Some("ok".into()),
            bypass_permissions: Some(false),
        };
        let json = serde_json::to_value(&val).unwrap();
        assert_eq!(json["bypassPermissions"], false);
        assert!(json.get("bypass_permissions").is_none());
    }

    // ── Elicitation ───────────────────────────────────────────────

    #[test]
    fn elicitation_deserializes() {
        let json = r#"{"variant":"elicitation","requestId":"e1","response":"42"}"#;
        let val: InteractRequest = serde_json::from_str(json).unwrap();
        match val {
            InteractRequest::Elicitation {
                request_id,
                response,
            } => {
                assert_eq!(request_id, "e1");
                assert_eq!(response, "42");
            }
            _ => panic!("expected Elicitation"),
        }
    }

    // ── Malformed input ───────────────────────────────────────────

    #[test]
    fn rejects_missing_variant() {
        let json = r#"{"requestId":"x","allowed":true}"#;
        let result = serde_json::from_str::<InteractRequest>(json);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_unknown_variant() {
        let json = r#"{"variant":"unknown_type","requestId":"x"}"#;
        let result = serde_json::from_str::<InteractRequest>(json);
        assert!(result.is_err());
    }
}
