use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::state::{AgentState, AgentStateGroup, SignalSource};

const TRANSIENT_EXPIRY_SECS: u64 = 60;

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

    pub(crate) fn is_expired(state: &str, elapsed: Duration) -> bool {
        match Self::state_category(state) {
            StateCategory::Terminal => false,
            StateCategory::Blocking => false,
            StateCategory::Transient => elapsed > Duration::from_secs(TRANSIENT_EXPIRY_SECS),
        }
    }

    pub async fn resolve(&self, session_id: &str) -> AgentState {
        if let Some((hook_state, timestamp)) = self.hook_states.read().await.get(session_id) {
            let expired = Self::is_expired(&hook_state.state, timestamp.elapsed());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::state::{AgentState, AgentStateGroup, SignalSource};

    fn make_hook_state(state: &str, group: AgentStateGroup) -> AgentState {
        AgentState {
            group,
            state: state.into(),
            label: "test".into(),
            confidence: 0.99,
            source: SignalSource::Hook,
            context: None,
        }
    }

    fn make_jsonl_state(state: &str, group: AgentStateGroup) -> AgentState {
        AgentState {
            group,
            state: state.into(),
            label: "test".into(),
            confidence: 0.5,
            source: SignalSource::Jsonl,
            context: None,
        }
    }

    // Pure function tests: expiry logic
    #[test]
    fn transient_not_expired_before_threshold() {
        assert!(!StateResolver::is_expired("acting", Duration::from_secs(59)));
        assert!(!StateResolver::is_expired("thinking", Duration::from_secs(0)));
        assert!(!StateResolver::is_expired("delegating", Duration::from_secs(30)));
    }

    #[test]
    fn transient_expired_after_threshold() {
        assert!(StateResolver::is_expired("acting", Duration::from_secs(61)));
        assert!(StateResolver::is_expired("thinking", Duration::from_secs(120)));
        assert!(StateResolver::is_expired("delegating", Duration::from_secs(3600)));
    }

    #[test]
    fn blocking_never_expires() {
        assert!(!StateResolver::is_expired("awaiting_input", Duration::from_secs(7200)));
        assert!(!StateResolver::is_expired("awaiting_approval", Duration::from_secs(86400)));
        assert!(!StateResolver::is_expired("needs_permission", Duration::from_secs(604800)));
        assert!(!StateResolver::is_expired("error", Duration::from_secs(7200)));
        assert!(!StateResolver::is_expired("idle", Duration::from_secs(7200)));
    }

    #[test]
    fn terminal_never_expires() {
        assert!(!StateResolver::is_expired("task_complete", Duration::from_secs(86400)));
        assert!(!StateResolver::is_expired("session_ended", Duration::from_secs(604800)));
    }

    // Integration tests: priority and cleanup
    #[tokio::test]
    async fn hook_state_takes_priority_over_jsonl() {
        let resolver = StateResolver::new();
        resolver.update_from_jsonl("s1", make_jsonl_state("idle", AgentStateGroup::NeedsYou)).await;
        resolver.update_from_hook("s1", make_hook_state("acting", AgentStateGroup::Autonomous)).await;
        let resolved = resolver.resolve("s1").await;
        assert_eq!(resolved.state, "acting");
        assert_eq!(resolved.group, AgentStateGroup::Autonomous);
    }

    #[tokio::test]
    async fn jsonl_state_used_when_no_hook() {
        let resolver = StateResolver::new();
        resolver.update_from_jsonl("s1", make_jsonl_state("idle", AgentStateGroup::NeedsYou)).await;
        let resolved = resolver.resolve("s1").await;
        assert_eq!(resolved.state, "idle");
    }

    #[tokio::test]
    async fn fallback_when_no_hook_or_jsonl() {
        let resolver = StateResolver::new();
        let resolved = resolver.resolve("nonexistent").await;
        assert_eq!(resolved.state, "unknown");
        assert_eq!(resolved.group, AgentStateGroup::Autonomous);
    }

    #[tokio::test]
    async fn cleanup_stale_removes_old_transient_keeps_blocking() {
        let resolver = StateResolver::new();
        resolver.update_from_hook("s1", make_hook_state("acting", AgentStateGroup::Autonomous)).await;
        resolver.update_from_hook("s2", make_hook_state("awaiting_input", AgentStateGroup::NeedsYou)).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        resolver.cleanup_stale(Duration::from_millis(10)).await;
        let s1 = resolver.resolve("s1").await;
        assert_eq!(s1.state, "unknown");
        let s2 = resolver.resolve("s2").await;
        assert_eq!(s2.state, "awaiting_input");
    }
}
