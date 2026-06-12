// crates/providers/src/model.rs
//
// Normalized output of every provider parser. The transcript itself is
// expressed in the shared ConversationBlock union (full tool structure
// preserved); this module adds the session-level metadata envelope.

use crate::kind::ProviderKind;
use claude_view_types::block_types::ConversationBlock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Anthropic-shape token totals. Parsers MUST normalize provider-native
/// accounting into these four buckets (e.g. OpenAI-style `prompt_tokens`
/// includes cached reads — subtract before filling `input_tokens`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub cache_creation_input_tokens: u64,
}

impl UsageTotals {
    /// Saturating accumulation — token counts come from untrusted foreign
    /// files; corrupt values must never panic (dev overflow checks) or wrap.
    pub fn add(&mut self, other: &UsageTotals) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_input_tokens = self
            .cache_read_input_tokens
            .saturating_add(other.cache_read_input_tokens);
        self.cache_creation_input_tokens = self
            .cache_creation_input_tokens
            .saturating_add(other.cache_creation_input_tokens);
    }

    pub fn is_zero(&self) -> bool {
        *self == UsageTotals::default()
    }
}

/// Session-level usage. `has_usage` is a truthful presence flag: false means
/// the format carries no token accounting at all (≠ "zero tokens used").
/// Per Trust-over-Accuracy the UI must show nothing in that case.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ForeignUsage {
    pub totals: UsageTotals,
    /// Keyed by raw model id — load-bearing for pricing lookup.
    pub per_model: HashMap<String, UsageTotals>,
    pub has_usage: bool,
}

impl ForeignUsage {
    /// Record one usage observation attributed to `model` (empty model id
    /// records into totals only — counted but unpriceable).
    pub fn record(&mut self, model: &str, u: UsageTotals) {
        self.totals.add(&u);
        self.has_usage = true;
        if !model.is_empty() {
            self.per_model.entry(model.to_string()).or_default().add(&u);
        }
    }
}

/// Session-level metadata extracted by a provider parser.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForeignSessionMeta {
    /// Namespaced id: `<provider>:<raw>`.
    pub id: String,
    pub provider: ProviderKind,
    /// Human-facing project name (provider-specific derivation: cwd basename,
    /// decoded dir name, workspace manifest, …).
    pub project: String,
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
    /// Agent-provided session title/name, when the format carries one.
    pub title: Option<String>,
    /// First real user message (preview text for cards).
    pub first_message: String,
    /// Epoch seconds.
    pub started_at: Option<f64>,
    pub ended_at: Option<f64>,
    pub message_count: u32,
    pub user_message_count: u32,
    /// Distinct model ids observed, newest-first not guaranteed.
    pub models: Vec<String>,
    pub usage: ForeignUsage,
    /// Real backing file, or `<db-path>#<raw-id>` virtual path for
    /// SQLite-backed providers.
    pub source_path: PathBuf,
    /// Tolerantly-skipped lines during parse. Surfaced, never hidden.
    pub malformed_lines: u32,
}

impl ForeignSessionMeta {
    pub fn new(provider: ProviderKind, raw_id: &str, source_path: PathBuf) -> Self {
        Self {
            id: provider.session_id(raw_id),
            provider,
            project: String::new(),
            cwd: None,
            git_branch: None,
            title: None,
            first_message: String::new(),
            started_at: None,
            ended_at: None,
            message_count: 0,
            user_message_count: 0,
            models: Vec::new(),
            usage: ForeignUsage::default(),
            source_path,
            malformed_lines: 0,
        }
    }

    /// Track a model id (dedup, preserve first-seen order).
    pub fn record_model(&mut self, model: &str) {
        if !model.is_empty() && !self.models.iter().any(|m| m == model) {
            self.models.push(model.to_string());
        }
    }

    /// Widen the [started_at, ended_at] envelope with an observation.
    pub fn observe_timestamp(&mut self, ts: f64) {
        if ts <= 0.0 {
            return;
        }
        match self.started_at {
            Some(s) if s <= ts => {}
            _ => self.started_at = Some(ts),
        }
        match self.ended_at {
            Some(e) if e >= ts => {}
            _ => self.ended_at = Some(ts),
        }
    }
}

/// One fully-parsed foreign session: metadata + the renderable transcript.
#[derive(Debug, Clone)]
pub struct ForeignSession {
    pub meta: ForeignSessionMeta,
    pub blocks: Vec<ConversationBlock>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_presence_is_truthful() {
        let mut u = ForeignUsage::default();
        assert!(!u.has_usage);
        u.record(
            "gpt-5.2-codex",
            UsageTotals {
                input_tokens: 10,
                output_tokens: 5,
                ..Default::default()
            },
        );
        assert!(u.has_usage);
        assert_eq!(u.totals.input_tokens, 10);
        assert_eq!(u.per_model["gpt-5.2-codex"].output_tokens, 5);
    }

    #[test]
    fn timestamp_envelope_widens() {
        let mut m = ForeignSessionMeta::new(ProviderKind::Amp, "t1", PathBuf::from("/x"));
        m.observe_timestamp(200.0);
        m.observe_timestamp(100.0);
        m.observe_timestamp(300.0);
        assert_eq!(m.started_at, Some(100.0));
        assert_eq!(m.ended_at, Some(300.0));
    }
}
