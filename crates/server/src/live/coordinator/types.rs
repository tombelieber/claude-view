//! Types and constants shared across the coordinator pipeline.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, RwLock};

use crate::live::manager::{LiveSessionManager, LiveSessionMap, TranscriptMap};
use crate::live::state::{HookEvent, SessionEvent, MAX_HOOK_EVENTS_PER_SESSION};

// ---------------------------------------------------------------------------
// MutationContext — borrows from AppState for the duration of one handle() call
// ---------------------------------------------------------------------------

/// Borrowed references to shared state needed by the mutation pipeline.
/// Created per-call from `AppState` fields — no ownership transfer.
pub struct MutationContext<'a> {
    pub sessions: &'a LiveSessionMap,
    pub live_tx: &'a broadcast::Sender<SessionEvent>,
    pub live_manager: Option<&'a Arc<LiveSessionManager>>,
    pub db: &'a claude_view_db::Database,
    pub transcript_to_session: &'a TranscriptMap,
    pub hook_event_channels: &'a Arc<RwLock<HashMap<String, broadcast::Sender<HookEvent>>>>,
    /// CLI session store for ownership resolution during broadcast.
    pub cli_sessions: &'a Arc<crate::routes::cli_sessions::store::CliSessionStore>,
    /// Side-map for full interaction data, keyed by request_id.
    pub interaction_data: &'a Arc<RwLock<HashMap<String, claude_view_types::InteractionBlock>>>,
}

// ---------------------------------------------------------------------------
// BufferedMutation — a mutation paired with its optional hook event
// ---------------------------------------------------------------------------

/// A buffered mutation with its associated hook event.
pub type BufferedMutation = (
    crate::live::mutation::types::SessionMutation,
    Option<HookEvent>,
);

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default buffer TTL for pending mutations (2 minutes).
pub const PENDING_TTL: Duration = Duration::from_secs(120);

/// Maximum hook events kept per session (re-exported for clarity).
pub const MAX_EVENTS: usize = MAX_HOOK_EVENTS_PER_SESSION;
