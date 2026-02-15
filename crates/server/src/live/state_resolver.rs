use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::state::{AgentState, AgentStateGroup, SignalSource};

/// Resolves the current AgentState for a session by merging hook and JSONL signals.
/// Hook signals take priority over JSONL-derived states.
#[derive(Clone)]
pub struct StateResolver {
    hook_states: Arc<RwLock<HashMap<String, (AgentState, Instant)>>>,
    jsonl_states: Arc<RwLock<HashMap<String, AgentState>>>,
}

impl StateResolver {
    pub fn new() -> Self {
        Self {
            hook_states: Arc::new(RwLock::new(HashMap::new())),
            jsonl_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_from_hook(&self, session_id: &str, state: AgentState) {
        self.hook_states.write().await
            .insert(session_id.to_string(), (state, Instant::now()));
    }

    pub async fn update_from_jsonl(&self, session_id: &str, state: AgentState) {
        self.jsonl_states.write().await
            .insert(session_id.to_string(), state);
    }

    pub async fn resolve(&self, session_id: &str) -> AgentState {
        if let Some((hook_state, timestamp)) = self.hook_states.read().await.get(session_id) {
            let expired = match Self::state_category(&hook_state.state) {
                StateCategory::Terminal => false,
                StateCategory::Blocking => false,
                StateCategory::Transient => timestamp.elapsed() > Duration::from_secs(60),
            };
            if !expired {
                return hook_state.clone();
            }
        }

        if let Some(jsonl_state) = self.jsonl_states.read().await.get(session_id) {
            return jsonl_state.clone();
        }

        AgentState {
            group: AgentStateGroup::Autonomous,
            state: "unknown".into(),
            label: "Status unavailable".into(),
            confidence: 0.0,
            source: SignalSource::Fallback,
            context: None,
        }
    }

    pub async fn cleanup_stale(&self, max_age: Duration) {
        let mut states = self.hook_states.write().await;
        states.retain(|_, (state, ts)| {
            match Self::state_category(&state.state) {
                StateCategory::Terminal => true,
                StateCategory::Blocking => true,
                StateCategory::Transient => ts.elapsed() < max_age,
            }
        });
    }

    fn state_category(state: &str) -> StateCategory {
        match state {
            "task_complete" | "session_ended" => StateCategory::Terminal,
            "awaiting_input" | "awaiting_approval" | "needs_permission" | "error" | "idle"
                => StateCategory::Blocking,
            _ => StateCategory::Transient,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StateCategory {
    Terminal,
    Blocking,
    Transient,
}
