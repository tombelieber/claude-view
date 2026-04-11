//! Pure function: apply an InteractionAction to the session's pending_interaction.
//!
//! Set stores compact meta; Clear removes it. Full interaction data is stored
//! in a side-map via SideEffect (Phase 3b), not here.

use claude_view_types::PendingInteractionMeta;

use crate::live::mutation::types::InteractionAction;

/// Apply an interaction action to the session's pending_interaction field.
///
/// Pure function -- no IO, no locks. Side-map writes happen via SideEffect.
pub fn apply_interaction(pending: &mut Option<PendingInteractionMeta>, action: &InteractionAction) {
    match action {
        InteractionAction::Set { meta, .. } => {
            *pending = Some(meta.clone());
        }
        InteractionAction::Clear { .. } => {
            *pending = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use claude_view_types::{InteractionVariant, PendingInteractionMeta};

    fn make_meta(request_id: &str) -> PendingInteractionMeta {
        PendingInteractionMeta {
            variant: InteractionVariant::Permission,
            request_id: request_id.to_string(),
            preview: "Allow file write?".to_string(),
        }
    }

    #[test]
    fn set_stores_meta() {
        let mut pending: Option<PendingInteractionMeta> = None;
        let meta = make_meta("req-001");
        let full_data = claude_view_types::InteractionBlock {
            id: "block-1".into(),
            variant: InteractionVariant::Permission,
            request_id: Some("req-001".into()),
            resolved: false,
            historical_source: None,
            data: serde_json::json!({"tool": "Bash", "command": "rm -rf /"}),
        };

        apply_interaction(
            &mut pending,
            &InteractionAction::Set {
                meta: meta.clone(),
                full_data,
            },
        );

        assert!(pending.is_some());
        let stored = pending.unwrap();
        assert_eq!(stored.request_id, "req-001");
        assert_eq!(stored.preview, "Allow file write?");
        assert!(matches!(stored.variant, InteractionVariant::Permission));
    }

    #[test]
    fn clear_removes_meta() {
        let mut pending: Option<PendingInteractionMeta> = Some(make_meta("req-001"));
        assert!(pending.is_some());

        apply_interaction(
            &mut pending,
            &InteractionAction::Clear {
                request_id: "req-001".into(),
            },
        );

        assert!(pending.is_none());
    }

    #[test]
    fn clear_is_idempotent_on_none() {
        let mut pending: Option<PendingInteractionMeta> = None;

        apply_interaction(
            &mut pending,
            &InteractionAction::Clear {
                request_id: "req-nonexistent".into(),
            },
        );

        assert!(pending.is_none());
    }

    #[test]
    fn set_overwrites_existing() {
        let mut pending: Option<PendingInteractionMeta> = Some(make_meta("req-001"));

        let new_meta = PendingInteractionMeta {
            variant: InteractionVariant::Question,
            request_id: "req-002".into(),
            preview: "What should I name the file?".into(),
        };
        let full_data = claude_view_types::InteractionBlock {
            id: "block-2".into(),
            variant: InteractionVariant::Question,
            request_id: Some("req-002".into()),
            resolved: false,
            historical_source: None,
            data: serde_json::json!({"question": "What name?"}),
        };

        apply_interaction(
            &mut pending,
            &InteractionAction::Set {
                meta: new_meta,
                full_data,
            },
        );

        let stored = pending.unwrap();
        assert_eq!(stored.request_id, "req-002");
        assert!(matches!(stored.variant, InteractionVariant::Question));
    }
}
