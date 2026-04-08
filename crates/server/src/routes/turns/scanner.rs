//! JSONL scanning logic for extracting per-turn breakdowns.
//!
//! Uses SIMD pre-filtering via `memchr` to avoid JSON parsing lines that
//! aren't interesting (user, assistant, system).

use memchr::memmem;

use claude_view_core::is_system_user_content;

use super::types::TurnInfo;

// ============================================================================
// SIMD Finders
// ============================================================================

/// Pre-compiled SIMD finders for turn scanning.
struct TurnFinders {
    type_user: memmem::Finder<'static>,
    type_assistant: memmem::Finder<'static>,
    type_system: memmem::Finder<'static>,
    tool_result: memmem::Finder<'static>,
    turn_duration: memmem::Finder<'static>,
    timestamp_key: memmem::Finder<'static>,
    content_key: memmem::Finder<'static>,
}

impl TurnFinders {
    fn new() -> Self {
        Self {
            type_user: memmem::Finder::new(b"\"type\":\"user\""),
            type_assistant: memmem::Finder::new(b"\"type\":\"assistant\""),
            type_system: memmem::Finder::new(b"\"type\":\"system\""),
            tool_result: memmem::Finder::new(b"\"tool_result\""),
            turn_duration: memmem::Finder::new(b"\"turn_duration\""),
            timestamp_key: memmem::Finder::new(b"\"timestamp\""),
            content_key: memmem::Finder::new(b"\"content\""),
        }
    }
}

// ============================================================================
// Extraction Helpers
// ============================================================================

/// Extract the first text content from a user line's raw bytes.
///
/// For user lines, content is either a JSON string (real prompt) or a JSON array
/// (tool_result continuation). We only care about string content.
fn extract_user_content(parsed: &serde_json::Value) -> Option<String> {
    let msg = parsed.get("message")?;
    let content = msg.get("content")?;

    match content {
        serde_json::Value::String(s) => Some(s.clone()),
        // Array content = tool_result blocks; handled by tool_result finder
        _ => None,
    }
}

/// Extract timestamp as unix seconds from a parsed JSONL line.
///
/// Claude Code uses either:
/// - ISO 8601 string: `"timestamp": "2026-01-28T10:00:00Z"`
/// - Unix epoch number: `"timestamp": 1700000000`
fn extract_timestamp(parsed: &serde_json::Value) -> Option<i64> {
    let ts = parsed.get("timestamp")?;
    match ts {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => {
            // Parse ISO 8601 to unix timestamp
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.timestamp())
                .or_else(|| {
                    // Try without timezone (assume UTC)
                    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
                        .ok()
                        .map(|ndt| ndt.and_utc().timestamp())
                })
        }
        _ => None,
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{}...", truncated)
    }
}

// ============================================================================
// Turn Scanner
// ============================================================================

/// Scan JSONL file and extract per-turn breakdown.
///
/// Uses SIMD pre-filtering to avoid JSON parsing lines that aren't interesting.
pub fn scan_turns(data: &[u8]) -> Vec<TurnInfo> {
    let finders = TurnFinders::new();

    // Intermediate state for building turns
    struct PendingTurn {
        index: u32,
        started_at: i64,
        prompt_preview: String,
        cc_duration_ms: Option<u64>,
    }

    let mut turns: Vec<TurnInfo> = Vec::new();
    let mut current_turn: Option<PendingTurn> = None;
    let mut last_timestamp: Option<i64> = None;
    let mut turn_counter: u32 = 0;

    for raw_line in data.split(|&b| b == b'\n') {
        if raw_line.is_empty() {
            continue;
        }

        let is_user = finders.type_user.find(raw_line).is_some();
        let is_assistant = finders.type_assistant.find(raw_line).is_some();
        let is_system = finders.type_system.find(raw_line).is_some();

        // We only need to parse lines that are user, assistant, or system
        if !is_user && !is_assistant && !is_system {
            continue;
        }

        // Check for turn_duration system message (SIMD pre-filter)
        if is_system && finders.turn_duration.find(raw_line).is_some() {
            if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                if let Some(ms) = parsed.get("durationMs").and_then(|v| v.as_u64()) {
                    // Attach duration to current turn
                    if let Some(ref mut turn) = current_turn {
                        turn.cc_duration_ms = Some(ms);
                    }
                }
                // Update last_timestamp from system message too
                if let Some(ts) = extract_timestamp(&parsed) {
                    last_timestamp = Some(ts);
                }
            }
            continue;
        }

        // For user lines: check if it's a real user turn start
        if is_user {
            // Skip tool_result continuations (SIMD check)
            if finders.tool_result.find(raw_line).is_some() {
                // Still extract timestamp for wall-clock tracking
                if finders.timestamp_key.find(raw_line).is_some() {
                    if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                        if let Some(ts) = extract_timestamp(&parsed) {
                            last_timestamp = Some(ts);
                        }
                    }
                }
                continue;
            }

            // Parse to check content
            if finders.content_key.find(raw_line).is_some() {
                if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                    let ts = extract_timestamp(&parsed);

                    // Check if this is a system-injected user message
                    if let Some(content) = extract_user_content(&parsed) {
                        if is_system_user_content(&content) {
                            // Not a real turn, but update timestamp
                            if let Some(t) = ts {
                                last_timestamp = Some(t);
                            }
                            continue;
                        }

                        // Check isMeta flag
                        if parsed
                            .get("isMeta")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            if let Some(t) = ts {
                                last_timestamp = Some(t);
                            }
                            continue;
                        }

                        // This is a real user turn start.
                        // Close the previous turn if one was open.
                        if let Some(pending) = current_turn.take() {
                            let end_ts = last_timestamp.unwrap_or(pending.started_at);
                            turns.push(TurnInfo {
                                index: pending.index,
                                started_at: pending.started_at,
                                wall_clock_seconds: (end_ts - pending.started_at).max(0),
                                cc_duration_ms: pending.cc_duration_ms,
                                prompt_preview: pending.prompt_preview,
                            });
                        }

                        turn_counter += 1;
                        let started_at = ts.unwrap_or(0);
                        let preview = truncate_preview(&content, 60);

                        current_turn = Some(PendingTurn {
                            index: turn_counter,
                            started_at,
                            prompt_preview: preview,
                            cc_duration_ms: None,
                        });

                        if let Some(t) = ts {
                            last_timestamp = Some(t);
                        }
                    } else {
                        // Content is array (tool_result) or missing — not a turn start
                        if let Some(t) = ts {
                            last_timestamp = Some(t);
                        }
                    }
                }
            }
            continue;
        }

        // For assistant/system lines: just track timestamp for wall-clock end
        if finders.timestamp_key.find(raw_line).is_some() {
            if let Ok(parsed) = serde_json::from_slice::<serde_json::Value>(raw_line) {
                if let Some(ts) = extract_timestamp(&parsed) {
                    last_timestamp = Some(ts);
                }
            }
        }
    }

    // Close the last turn
    if let Some(pending) = current_turn.take() {
        let end_ts = last_timestamp.unwrap_or(pending.started_at);
        turns.push(TurnInfo {
            index: pending.index,
            started_at: pending.started_at,
            wall_clock_seconds: (end_ts - pending.started_at).max(0),
            cc_duration_ms: pending.cc_duration_ms,
            prompt_preview: pending.prompt_preview,
        });
    }

    turns
}
