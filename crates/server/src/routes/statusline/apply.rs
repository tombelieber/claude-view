//! Apply statusline payload fields to a live session.
//!
//! Delegates field merges to the pure `apply_statusline` on StatuslineFields,
//! then handles cross-source fields (model, context_window_tokens) on LiveSession.

use crate::live::state::LiveSession;

use super::types::StatuslinePayload;

/// Apply statusline payload fields to a live session.
pub fn apply_statusline(session: &mut LiveSession, payload: &StatuslinePayload) {
    // Delegate all 32 statusline fields to the sub-struct
    crate::live::mutation::apply_statusline::apply_statusline(&mut session.statusline, payload);

    // context_window_tokens lives on LiveSession (derived from current_usage)
    if let Some(ref cw) = payload.context_window {
        if let Some(ref usage) = cw.current_usage {
            let fill = usage.input_tokens.unwrap_or(0)
                + usage.cache_creation_input_tokens.unwrap_or(0)
                + usage.cache_read_input_tokens.unwrap_or(0);
            if fill > 0 {
                session.context_window_tokens = fill;
            }
        }
    }

    // Model -- timestamp-guarded to prevent stale statusline from overwriting
    // a newer hook value. Statusline is authoritative for model (it reflects
    // mid-session model switches via /model command), but only if it's fresher.
    if let Some(ref m) = payload.model {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if now > session.model_set_at {
            if let Some(ref id) = m.id {
                if !id.is_empty() {
                    session.model = Some(id.clone());
                    session.model_set_at = now;
                }
            }
            if let Some(ref dn) = m.display_name {
                if !dn.is_empty() {
                    session.model_display_name = Some(dn.clone());
                }
            }
        }
    }
}
