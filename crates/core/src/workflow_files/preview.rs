//! Text preview + secret redaction.
//!
//! Every string produced from artifact *content* (scripts, results, summaries,
//! agent transcripts, journal payloads, file previews) is funneled through
//! [`safe_preview`] / [`preview_value`], which truncate and then redact. This is
//! the single trust boundary: a preview field must never serialize unredacted.
//! Identifiers/labels (run id, project dir, phase title) are NOT routed here.

use std::sync::OnceLock;

use regex_lite::Regex;
use serde_json::Value;

const REDACTED: &str = "[redacted]";

/// Char-bounded truncation (raw — no redaction). Operates on `char`s so it never
/// splits a UTF-8 sequence. Used for both content (via `safe_preview`) and for
/// non-secret identifiers/labels.
pub(crate) fn truncate(text: &str, limit: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in text.chars().enumerate() {
        if idx >= limit {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

/// Truncate then redact — the canonical way to build a content preview string.
pub(crate) fn safe_preview(text: &str, limit: usize) -> String {
    redact_secret_like_text(&truncate(text, limit))
}

/// High-signal secret token shapes (provider key prefixes, JWTs, bearer tokens,
/// PEM markers). Matched substrings are replaced wholesale.
fn token_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----",
            r"(?i)bearer\s+[A-Za-z0-9._~+/=-]{12,}",
            r"sk-ant-[A-Za-z0-9_-]{12,}",
            r"sk-[A-Za-z0-9_-]{16,}",
            r"gh[posru]_[A-Za-z0-9]{20,}",
            r"github_pat_[A-Za-z0-9_]{20,}",
            r"xox[abprs]-[A-Za-z0-9-]{10,}",
            r"glpat-[A-Za-z0-9_-]{16,}",
            r"AKIA[0-9A-Z]{16}",
            r"ASIA[0-9A-Z]{16}",
            r"AIza[0-9A-Za-z_-]{30,}",
            r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{6,}",
        ]
        .iter()
        .map(|p| Regex::new(p).expect("static redaction pattern compiles"))
        .collect()
    })
}

/// `key = value` / `"key": "value"` assignments where the key *ends in* a
/// credential word (so `API_TOKEN`, `db_password`, `clientSecret` all match,
/// while prose like "the authorization flow" or "author: Tom" does not). Only the
/// value is redacted, and only when it is at least 6 non-delimiter chars long —
/// the length gate keeps short prose values (`Token: how`) readable. Group 1 is
/// the key + separator (+ any opening quote); group 2 is the value.
fn keyvalue_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)("?[A-Za-z0-9_-]*(?:api[_-]?key|apikey|secret|token|password|passwd|authorization|access[_-]?token|refresh[_-]?token|client[_-]?secret|private[_-]?key)"?\s*[:=]\s*"?)([^\s"',}]{6,})"#,
        )
        .expect("static key-value redaction pattern compiles")
    })
}

/// Redact secret-like substrings. Token patterns run first (so `Bearer <tok>` is
/// caught whole), then credential `key: value` assignments. Idempotent.
pub(crate) fn redact_secret_like_text(text: &str) -> String {
    let mut out = text.to_string();
    for pattern in token_patterns() {
        if pattern.is_match(&out) {
            out = pattern.replace_all(&out, REDACTED).into_owned();
        }
    }
    out = keyvalue_pattern()
        .replace_all(&out, format!("${{1}}{REDACTED}").as_str())
        .into_owned();
    out
}

/// Build a redacted preview from an arbitrary JSON value.
pub(crate) fn preview_value(value: &Value, limit: usize) -> String {
    match value {
        Value::String(text) => safe_preview(text, limit),
        _ => {
            let serialized = serde_json::to_string_pretty(value)
                .or_else(|_| serde_json::to_string(value))
                .unwrap_or_default();
            safe_preview(&serialized, limit)
        }
    }
}

/// Build a redacted preview from a Claude message `content` field (string or the
/// array-of-blocks shape), flattening text + tool blocks.
pub(crate) fn preview_message_content(value: &Value, limit: usize) -> String {
    match value {
        Value::String(text) => safe_preview(text, limit),
        Value::Array(items) => {
            let mut parts = Vec::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    parts.push(text.to_string());
                } else if let Some(name) = item.get("name").and_then(Value::as_str) {
                    parts.push(format!(
                        "{} {}",
                        item.get("type").and_then(Value::as_str).unwrap_or("tool"),
                        name
                    ));
                } else {
                    parts.push(preview_value(item, limit));
                }
            }
            safe_preview(&parts.join("\n"), limit)
        }
        _ => preview_value(value, limit),
    }
}
