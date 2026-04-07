//! Private helper functions and finalization logic for the session accumulator.

use std::collections::HashMap;
use std::path::Path;

use crate::live_parser::{parse_tail, TailFinders};
use crate::phase::{dominant_phase, PhaseHistory};
use crate::pricing::{finalize_cost_breakdown, CacheStatus, ModelPricing};

use super::types::{RichSessionData, SessionAccumulator};

impl SessionAccumulator {
    /// Finalize accumulation: return per-turn accumulated cost, derive cache
    /// status, combine progress items, and return the complete [`RichSessionData`].
    ///
    /// Note: `pricing` is kept in the signature for backward compatibility but is
    /// no longer used -- cost is accumulated per-turn in `process_line()`.
    pub fn finish(&self, _pricing: &HashMap<String, ModelPricing>) -> RichSessionData {
        let cache_status = derive_cache_status(self.last_cache_hit_at);

        let mut progress_items = self.todo_items.clone();
        progress_items.extend(self.task_items.clone());
        let mut cost = self.accumulated_cost.clone();
        finalize_cost_breakdown(&mut cost, &self.tokens);

        RichSessionData {
            tokens: self.tokens.clone(),
            cost,
            cache_status,
            sub_agents: self.sub_agents.clone(),
            team_name: self.team_name.clone(),
            progress_items,
            context_window_tokens: self.context_window_tokens,
            model: self.model.clone(),
            git_branch: self.git_branch.clone(),
            turn_count: self.user_turn_count,
            first_user_message: if self.first_user_message.is_empty() {
                None
            } else {
                Some(self.first_user_message.clone())
            },
            last_user_message: if self.last_user_message.is_empty() {
                None
            } else {
                Some(self.last_user_message.clone())
            },
            last_cache_hit_at: self.last_cache_hit_at,
            slug: self.slug.clone(),
            phase: PhaseHistory {
                current: self.phase_labels.last().cloned(),
                dominant: dominant_phase(&self.phase_labels),
                labels: self.phase_labels.clone(),
                freshness: Default::default(),
            },
        }
    }

    /// Convenience: read a JSONL file from offset 0, parse all lines, and
    /// return the accumulated [`RichSessionData`].
    ///
    /// Uses synchronous I/O internally (suitable for `spawn_blocking`).
    pub fn from_file(
        path: &Path,
        pricing: &HashMap<String, ModelPricing>,
    ) -> std::io::Result<RichSessionData> {
        let finders = TailFinders::new();
        let (lines, _offset) = parse_tail(path, 0, &finders)?;

        // Derive a fallback activity timestamp from the file's mtime
        let last_activity_at = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            })
            .unwrap_or(0);

        let mut acc = Self::new();
        for line in &lines {
            acc.process_line(line, last_activity_at, pricing);
        }
        Ok(acc.finish(pricing))
    }
}

// =============================================================================
// Private helpers
// =============================================================================

/// Derive cache status from the last cache hit timestamp.
///
/// Warm if the last cache activity was within 5 minutes, Cold otherwise.
pub(crate) fn derive_cache_status(last_cache_hit_at: Option<i64>) -> CacheStatus {
    match last_cache_hit_at {
        Some(ts) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            if (now - ts) < 300 {
                CacheStatus::Warm
            } else {
                CacheStatus::Cold
            }
        }
        None => CacheStatus::Unknown,
    }
}

/// Parse an ISO 8601 / RFC 3339 timestamp to a Unix epoch second.
pub(crate) fn parse_timestamp_to_unix(ts: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp())
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc().timestamp())
        })
}
