// crates/providers/src/catalog.rs
//
// ForeignCatalog — in-memory index of discovered foreign sessions plus a
// fingerprint-keyed stats cache.
//
// Discovery is a cheap stat-walk (the server refreshes it periodically from
// its own task); session METADATA is parsed lazily on first request and
// cached keyed on (mtime, size) — the skip strategy proven by agentsview's
// sync engine. Full transcripts are parsed on view, like the CC pipeline.

use crate::discover::{registry, stat_entry, DiscoveredSession};
use crate::kind::ProviderKind;
use crate::model::{ForeignSession, ForeignSessionMeta};
use std::collections::HashMap;
use std::sync::RwLock;

/// Per-session searchable text cap. Transcripts are joined block text,
/// lowercased and truncated — enough for honest full-text matching without
/// holding whole corpora in memory (600 sessions × 64KB ≈ 38MB worst case).
const SEARCH_TEXT_CAP: usize = 64 * 1024;

#[derive(Default)]
struct Inner {
    /// id → discovered row (refreshed wholesale).
    rows: HashMap<String, DiscoveredSession>,
    /// id → (fingerprint, parsed meta).
    stats: HashMap<String, ((f64, u64), ForeignSessionMeta)>,
    /// id → lowercased transcript text (capped) for q= search. Built from
    /// the blocks already in hand when a session is parsed for its meta —
    /// no extra parse. Queried under the lock, never cloned out.
    search_texts: HashMap<String, String>,
}

/// Thread-safe foreign-session index. One instance lives in server AppState.
#[derive(Default)]
pub struct ForeignCatalog {
    inner: RwLock<Inner>,
}

impl ForeignCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-run discovery across every registered provider's roots. Replaces
    /// the row set; keeps the stats cache (still keyed by fingerprint).
    /// Returns the number of sessions discovered.
    pub fn refresh(&self) -> usize {
        let mut rows = HashMap::new();
        for provider in registry() {
            for root in provider.kind().session_roots() {
                for s in provider.discover(&root) {
                    rows.insert(s.id.clone(), s);
                }
            }
        }
        let n = rows.len();
        let mut inner = self.inner.write().expect("catalog lock poisoned");
        // Drop cache entries whose session vanished.
        inner.stats.retain(|id, _| rows.contains_key(id));
        inner.search_texts.retain(|id, _| rows.contains_key(id));
        inner.rows = rows;
        n
    }

    /// Number of discovered sessions.
    pub fn len(&self) -> usize {
        self.inner.read().expect("catalog lock poisoned").rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Look up one discovered row by namespaced id.
    pub fn get(&self, id: &str) -> Option<DiscoveredSession> {
        self.inner
            .read()
            .expect("catalog lock poisoned")
            .rows
            .get(id)
            .cloned()
    }

    /// All discovered rows (cheap clone of stat-level data).
    pub fn rows(&self) -> Vec<DiscoveredSession> {
        self.inner
            .read()
            .expect("catalog lock poisoned")
            .rows
            .values()
            .cloned()
            .collect()
    }

    /// Session metadata for every discovered session, parsing only sessions
    /// whose (mtime, size) fingerprint changed since the last call.
    ///
    /// Sessions that fail to parse are skipped (logged) — a broken foreign
    /// file must never take the list down.
    pub fn list_meta(&self) -> Vec<ForeignSessionMeta> {
        let rows = self.rows();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(meta) = self.meta_for(&row) {
                out.push(meta);
            }
        }
        out
    }

    /// Metadata for one session, via cache or fresh parse.
    pub fn meta_for(&self, row: &DiscoveredSession) -> Option<ForeignSessionMeta> {
        let fingerprint = current_fingerprint(row);
        {
            let inner = self.inner.read().expect("catalog lock poisoned");
            if let Some((fp, meta)) = inner.stats.get(&row.id) {
                if *fp == fingerprint {
                    return Some(meta.clone());
                }
            }
        }
        let sessions = self.parse_row(row)?;
        let mut wanted = None;
        let mut inner = self.inner.write().expect("catalog lock poisoned");
        for s in sessions {
            if s.meta.id == row.id {
                wanted = Some(s.meta.clone());
            }
            inner
                .search_texts
                .insert(s.meta.id.clone(), searchable_text(&s.blocks));
            inner.stats.insert(s.meta.id.clone(), (fingerprint, s.meta));
        }
        wanted
    }

    /// True when the session's cached transcript text contains
    /// `query_lower` (caller lowercases). Checked under the read lock —
    /// the text never leaves the catalog.
    pub fn transcript_matches(&self, id: &str, query_lower: &str) -> bool {
        self.inner
            .read()
            .expect("catalog lock poisoned")
            .search_texts
            .get(id)
            .is_some_and(|t| t.contains(query_lower))
    }

    /// Full parse (blocks included) for the messages route.
    pub fn parse_session(&self, id: &str) -> Option<ForeignSession> {
        let row = self.get(id)?;
        let sessions = self.parse_row(&row)?;
        sessions.into_iter().find(|s| s.meta.id == id)
    }

    fn parse_row(&self, row: &DiscoveredSession) -> Option<Vec<ForeignSession>> {
        let provider = crate::discover::provider_for(row.provider)?;
        match provider.parse(&row.path) {
            Ok(sessions) => Some(sessions),
            Err(e) => {
                tracing::warn!(
                    session = %row.id,
                    path = %row.path.display(),
                    error = %e,
                    "foreign session parse failed; skipping"
                );
                None
            }
        }
    }
}

/// Live fingerprint for a row: re-stat real files (a session may have grown
/// since discovery); virtual `db#id` paths fall back to discovery-time data.
fn current_fingerprint(row: &DiscoveredSession) -> (f64, u64) {
    if crate::discover::split_virtual_path(&row.path).is_none() {
        if let Some(fp) = stat_entry(&row.path) {
            return fp;
        }
    }
    (row.mtime, row.size_bytes)
}

/// Join a transcript's visible text (user text, assistant text/thinking,
/// tool names) into one lowercased haystack, capped at [`SEARCH_TEXT_CAP`].
fn searchable_text(blocks: &[claude_view_types::block_types::ConversationBlock]) -> String {
    use claude_view_types::block_types::{AssistantSegment, ConversationBlock};
    let mut out = String::new();
    let push = |out: &mut String, s: &str| {
        if out.len() < SEARCH_TEXT_CAP && !s.is_empty() {
            out.push_str(s);
            out.push('\n');
        }
    };
    for block in blocks {
        match block {
            ConversationBlock::User(u) => push(&mut out, &u.text),
            ConversationBlock::Assistant(a) => {
                if let Some(t) = &a.thinking {
                    push(&mut out, t);
                }
                for seg in &a.segments {
                    match seg {
                        AssistantSegment::Text { text, .. } => push(&mut out, text),
                        AssistantSegment::Tool { execution } => {
                            push(&mut out, &execution.tool_name)
                        }
                    }
                }
            }
            _ => {}
        }
        if out.len() >= SEARCH_TEXT_CAP {
            break;
        }
    }
    // Truncate on a char boundary — String::truncate panics mid-codepoint.
    if out.len() > SEARCH_TEXT_CAP {
        let mut cut = SEARCH_TEXT_CAP;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
    }
    out.to_lowercase()
}

/// Providers visible in this build, for the settings/diagnostics surface.
pub fn supported_providers() -> Vec<(ProviderKind, &'static str)> {
    registry()
        .iter()
        .map(|p| (p.kind(), p.kind().display_name()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::blocks;

    #[test]
    fn searchable_text_joins_lowercases_and_caps() {
        let blocks = vec![
            blocks::user("u0".into(), "Fix the LOGIN bug".into(), None),
            blocks::assistant(
                "a0".into(),
                vec![
                    blocks::text_segment("Checking Auth.ts now".into()),
                    blocks::tool_segment("Read".into(), serde_json::json!({}), "t1".into()),
                ],
                Some("the JWT path looks wrong".into()),
                None,
            ),
        ];
        let text = searchable_text(&blocks);
        assert!(text.contains("fix the login bug"));
        assert!(text.contains("checking auth.ts"));
        assert!(text.contains("jwt path"));
        assert!(text.contains("read"));
        assert!(!text.contains("LOGIN"), "must be lowercased");
    }

    #[test]
    fn searchable_text_truncates_on_char_boundary() {
        // Multi-byte content larger than the cap must not panic.
        let big = "界".repeat(SEARCH_TEXT_CAP); // 3 bytes per char
        let blocks = vec![blocks::user("u0".into(), big, None)];
        let text = searchable_text(&blocks);
        assert!(text.len() <= SEARCH_TEXT_CAP);
        assert!(text.contains('界'));
    }
}
