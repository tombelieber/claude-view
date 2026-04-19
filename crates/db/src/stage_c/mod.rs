//! Stage C — rollup-table writer for CQRS Phase 4.
//!
//! Consumes `StatsDelta` (already-parsed session observations from
//! indexer-v2 / live-tail / drift-healer) and fans out 12 UPSERT
//! statements per delta — one per `(bucket × dimension)` pair where
//! `dimension != category`. Category dimension is deferred to Phase 5
//! because it depends on the `SessionFlags` LWW fold for classification
//! labels.
//!
//! ## Where this fits in the data flow
//!
//! ```text
//!    ┌──────────────┐      ┌──────────────┐
//!    │ indexer-v2   │──────│ live-tail    │  (producers)
//!    └──────┬───────┘      └──────┬───────┘
//!           │      StatsDelta     │
//!           └──────────┬──────────┘
//!                      ▼
//!              ┌─────────────────┐
//!              │  Stage C (here) │
//!              └────┬────────────┘
//!                   │
//!            12× UPSERT per delta
//!                   │
//!        ┌──────────┴──────────────────┐
//!        ▼                             ▼
//!   daily_*_stats                 weekly_*_stats, monthly_*_stats
//! ```
//!
//! ## What is NOT in this module (deferred)
//!
//! - **`FlagDelta` apply path**: compensating UPDATEs when
//!   `SessionFlags` changes (archive toggle, category reclassify).
//!   Needs Phase 5 `session_flags` LWW fold first. When it lands, it
//!   goes in `stage_c/flag_delta.rs`.
//! - **Durable outbox**: `stage_c_outbox` table + drain task for
//!   crash-safe `FlagDelta` delivery. Needs `FlagDelta` first. Lands
//!   in `stage_c/outbox.rs`.
//! - **Server startup wiring**: `spawn_stage_c` integration into
//!   `AppState`/`app_factory.rs`. Phase 4b follow-up.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §6.2 PR 4.2`.

pub mod consumer;
pub mod rebuild;

pub use consumer::{apply_stats_delta, resolve_observation_ts, StageCError};
pub use rebuild::{full_rebuild_from_session_stats, RebuildSummary};
