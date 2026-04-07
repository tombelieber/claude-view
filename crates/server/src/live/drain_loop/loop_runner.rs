use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinSet;

use crate::live::manager::accumulator::SessionAccumulator;
use crate::local_llm::client::LlmClient;
use claude_view_core::phase::scheduler::ClassifyResult;
use claude_view_core::phase::SessionPhase;

use super::state::DrainState;
use super::types::DirtySignal;

/// Run the drain loop as a long-lived tokio task.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_drain_loop(
    mut dirty_rx: mpsc::Receiver<DirtySignal>,
    result_tx: mpsc::Sender<ClassifyResult>,
    accumulators: Arc<RwLock<HashMap<String, SessionAccumulator>>>,
    client: Arc<LlmClient>,
    llm_ready: Arc<AtomicBool>,
    wake: Arc<Notify>,
    sessions: Arc<RwLock<HashMap<String, crate::live::state::LiveSession>>>,
    tx: tokio::sync::broadcast::Sender<crate::live::state::SessionEvent>,
    mode_multiplier: f32,
) {
    let mut state = DrainState {
        dirty: HashMap::new(),
        error_streak: 0,
        last_error_at: None,
        accumulators,
        client,
        result_tx,
        llm_ready,
        sessions,
        tx,
        mode_multiplier,
        avg_latency_ms: 400.0,
    };
    let mut tasks: JoinSet<(String, bool, Option<SessionPhase>, u64)> = JoinSet::new();

    loop {
        tokio::select! {
            msg = dirty_rx.recv() => {
                let Some(signal) = msg else { break };
                match signal {
                    DirtySignal::Activity(session_id, priority) => {
                        state.mark_dirty(session_id, priority);
                    }
                    DirtySignal::UserTurn(session_id, priority) => {
                        state.mark_dirty(session_id.clone(), priority);
                        state.signal_user_turn(&session_id);
                    }
                }
                state.try_drain(&mut tasks).await;
            }

            Some(result) = tasks.join_next() => {
                state.handle_completion(result);
                state.try_drain(&mut tasks).await;
            }

            _ = wake.notified() => {
                state.try_drain(&mut tasks).await;
            }

            _ = tokio::time::sleep(Duration::from_millis(500)) => {
                state.broadcast_pending().await;
                state.settle_idle_sessions().await;
                state.try_drain(&mut tasks).await;
            }
        }
    }

    tasks.shutdown().await;
}
