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

#[derive(Default)]
struct Inner {
    /// id → discovered row (refreshed wholesale).
    rows: HashMap<String, DiscoveredSession>,
    /// id → (fingerprint, parsed meta).
    stats: HashMap<String, ((f64, u64), ForeignSessionMeta)>,
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
            inner.stats.insert(s.meta.id.clone(), (fingerprint, s.meta));
        }
        wanted
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

/// Providers visible in this build, for the settings/diagnostics surface.
pub fn supported_providers() -> Vec<(ProviderKind, &'static str)> {
    registry()
        .iter()
        .map(|p| (p.kind(), p.kind().display_name()))
        .collect()
}
