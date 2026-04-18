//! Phase 2 indexer_v2 — shared types + tunables.
//!
//! `StatsDelta` is the single message format on the writer channel. The
//! orchestrator parses + extracts a [`SessionStats`] and packages it
//! together with the staleness header that lets `get_stats_header` skip
//! re-parsing on the next fsnotify event for the same file.

use claude_view_session_parser::SessionStats;

/// Fsnotify event coalesce window — applied per-`session_id` so a burst
/// of writes within `DEBOUNCE_MS` only triggers a single re-index.
///
/// 500 ms is the D1 sign-off value (Phase 1-7 design doc §1).
pub const DEBOUNCE_MS: u64 = 500;

/// Bounded capacity for the shared `StatsDelta` mpsc channel.
///
/// Back-pressure contract (SOTA §10 Phase 2.5):
/// - Producers (indexer_v2 orchestrator, live-tail watcher, drift healer)
///   use `try_send` only; they never block their own event loops on a
///   slow consumer.
/// - Drops on `TrySendError::Full` bump
///   `stage_c_producer_drop_total{producer=<source>}` so operators see
///   overflow without instrumenting the consumer.
/// - Drops are recoverable: the fsnotify shadow indexer re-indexes every
///   changed file within `DEBOUNCE_MS`, so a dropped live-tail packet is
///   covered on the next tick.
///
/// 1024 at typical live-session event rate (2-5 Hz) gives 200-500 s of
/// buffering before drops begin — ample headroom for pathological bursts
/// (full rebuild storming the channel).
pub const STATS_DELTA_CHANNEL_CAPACITY: usize = 1024;

/// Origin of a [`StatsDelta`].
///
/// Carried through the writer path so the Phase 4 Stage C consumer and
/// the `/metrics` endpoint can label drop counters and coalesce
/// decisions per producer. Phase 2.5 reads the label off
/// `stage_c_producer_drop_total`; later phases consume `old`/`new` for
/// rollup deltas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaSource {
    /// Indexer_v2 fsnotify shadow indexer.
    Indexer,
    /// Live-session tail parser (`server/live/manager/watcher.rs`).
    LiveTail,
    /// Drift healer — Phase 7 reconciliation after drift detector fires.
    DriftHealer,
}

impl DeltaSource {
    /// Label used for the `stage_c_producer_drop_total{producer}` metric
    /// and any other per-producer observability surface.
    pub const fn metric_label(self) -> &'static str {
        match self {
            DeltaSource::Indexer => "indexer",
            DeltaSource::LiveTail => "live_tail",
            DeltaSource::DriftHealer => "drift_healer",
        }
    }
}

/// Single payload pushed onto the writer channel after a successful
/// parse + extract. Owned (no borrows) so it's `Send + 'static` for
/// `mpsc::Sender::send`.
///
/// Two concern groups carried by the same struct (Phase 2.5 D2 decision:
/// extend the existing struct rather than split into two — keeps the
/// module surface smallest):
///
/// 1. **Writer payload** (`source_*`, `stats`): consumed by
///    [`super::writer::upsert_session_stats`] to produce one
///    `session_stats` row. Present in every delta.
/// 2. **Lineage** (`old`, `seq`, `source`): consumed by the Phase 4
///    Stage C rollup consumer and by `/metrics` drop counters. Writer
///    ignores these; producers populate them for observability.
#[derive(Debug, Clone)]
pub struct StatsDelta {
    /// Session UUID — matches `session_stats.session_id`.
    pub session_id: String,
    /// blake3 head+tail of the source JSONL bytes at parse time.
    pub source_content_hash: Vec<u8>,
    /// Byte length of the source JSONL at parse time.
    pub source_size: i64,
    /// Filesystem inode at parse time (None on platforms without inodes).
    pub source_inode: Option<i64>,
    /// blake3 of the file's mid 64 KB (only computed for files >1 MiB).
    pub source_mid_hash: Option<Vec<u8>>,
    /// Parsed + extracted statistics that populate the 24 stats columns.
    /// Matches the Phase 2.5 design's `new: SessionStats` — the writer
    /// persists this.
    pub stats: SessionStats,

    // ── Phase 2.5 lineage (reserved for Phase 4 Stage C rollups) ──
    /// Previous `SessionStats` for this session, if the producer can
    /// compute it cheaply. `None` on first observation (indexer cold
    /// start) or when the producer opts out. Not consumed in Phase 2.5;
    /// Phase 4 Stage C uses it for rollup delta emission.
    pub old: Option<SessionStats>,
    /// Monotonic sequence number per producer. Phase 2.5 consumers only
    /// log this; Phase 4 Stage C uses it for ordering + coalesce dedup.
    pub seq: u64,
    /// Producer identity — drives labeled drop metrics and per-producer
    /// observability.
    pub source: DeltaSource,
}

#[cfg(test)]
mod tests {
    use super::DeltaSource;

    /// The `stage_c_producer_drop_total{producer=…}` labels are a public
    /// contract: the Prometheus scraper + dashboards key off these
    /// strings. Renaming a variant's label silently breaks alerts, so
    /// we pin the mapping here.
    #[test]
    fn delta_source_metric_labels_are_the_sota_contract() {
        assert_eq!(DeltaSource::Indexer.metric_label(), "indexer");
        assert_eq!(DeltaSource::LiveTail.metric_label(), "live_tail");
        assert_eq!(DeltaSource::DriftHealer.metric_label(), "drift_healer");
    }
}
