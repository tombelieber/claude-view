//! Event-log migrations — append-only event-sourcing tables.
//!
//! Starts at migration 82 (CQRS Phase 5 PR 5.1). Placed AFTER `rollups`
//! in the canonical apply order so the proc-macro-generated rollup slice
//! keeps its contiguous version range and this module becomes the new
//! trailing append point.
//!
//! Migration ordering is load-bearing. Append-only — never insert in the
//! middle and never reorder. A `_migrations`-tracked production database
//! at version 81 (rollups tail) applies migration 82 only; an empty DB
//! applies every prior migration first, then lands here at v82.
//!
//! See `private/config/docs/plans/2026-04-17-cqrs-phase-1-7-design.md §7.1`
//! for the Phase 5 event-log design. Fold writer + dual-write handlers
//! ship in subsequent Phase 5 PRs (5.2 / 5.3) on top of this schema.

pub const MIGRATIONS: &[&str] = &[
    // Migration 82: session_action_log — append-only event log for
    // session mutations. Every archive / unarchive / classify / dismiss /
    // reclassify mutation lands here as a single immutable row inside
    // the same transaction as the legacy `sessions` column write
    // (PR 5.2 adds dual-write). PR 5.3's fold task consumes this log
    // into `session_flags` and advances `applied_seq` atomically.
    //
    // Column design (per §7.1):
    //   seq         — monotonic sequence. Primary key + AUTOINCREMENT so
    //                 `applied_seq` advances strictly and a fold-task
    //                 restart can resume from `SELECT MAX(applied_seq)`
    //                 without scanning by timestamp (timestamps collide
    //                 at second granularity under burst writes; see
    //                 memory entry `feedback_check_existing_before_adding`).
    //   session_id  — TEXT (matches `sessions.id` / `session_flags.session_id`).
    //                 No FK — §10 of the SOTA plan treats the legacy
    //                 `sessions` table as Phase-6 retirement scope, so a
    //                 hard FK here would block Phase 6. Cross-table
    //                 integrity is enforced by the fold writer + a
    //                 Phase 7 drift detector.
    //   action      — 'archive' | 'unarchive' | 'classify' | 'dismiss' |
    //                 'reclassify'. Plain TEXT (not CHECK-constrained)
    //                 so Phase 5.5+ can introduce new actions without a
    //                 migration; Rust-side enum serialisation is the
    //                 authoritative validator.
    //   payload     — JSON blob. `{}` for archive/unarchive/dismiss;
    //                 `ClassifyPayload { l1, l2, l3, confidence, source }`
    //                 for classify / reclassify.
    //   actor       — 'user' | 'classifier:<model-id>' | 'system:<reason>'.
    //                 Keeps audit-trail free-form so a new classifier
    //                 model or a one-off admin script needs no migration.
    //   at          — unix ms. INTEGER per STRICT-mode convention.
    //
    // Indexes:
    //   idx_action_session  : per-session history queries (UI "why was
    //                         this session archived?" trail, regression
    //                         investigation). Ordered (session_id, at)
    //                         so the common case (one session, sort by
    //                         time) is a zero-sort range scan.
    //   idx_action_actor_at : actor-filtered audit queries ("what did
    //                         classifier:haiku-4.5 do over the last
    //                         hour?"). Used by the Phase 5 classifier
    //                         retry / idempotency check and by Phase 7
    //                         observability dashboards.
    //
    // STRICT enforces the INTEGER / TEXT column types at INSERT time;
    // without it SQLite silently widens to ANY and a bad serialiser
    // could land a string `at`, which would corrupt the fold ordering.
    r#"BEGIN;
CREATE TABLE session_action_log (
    seq         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id  TEXT    NOT NULL,
    action      TEXT    NOT NULL,
    payload     TEXT    NOT NULL,
    actor       TEXT    NOT NULL,
    at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_action_session ON session_action_log(session_id, at);
CREATE INDEX idx_action_actor_at ON session_action_log(actor, at);
COMMIT;"#,
    // Migration 83: fold_state — single-row watermark for PR 5.3's
    // `spawn_flags_fold` task. Stores the max `session_action_log.seq`
    // the fold has applied to `session_flags`. Advanced in the SAME
    // transaction as the `session_flags` UPSERT so a kill-9 mid-batch
    // cannot leave the fold further ahead than the watermark (§7.2
    // kill-9 property: fold is resumable and byte-identical to a
    // one-shot replay).
    //
    // Why a single row instead of a COUNT()-derived watermark:
    //   - SELECT COUNT is O(log N) with sqlite's B-tree, but reading a
    //     single scalar is O(1) and cheaper by a constant factor.
    //   - Atomic UPDATE .. WHERE id = 0 keeps the watermark contract
    //     inside the same TX as the fold UPSERTs; COUNT-derived
    //     watermarks race with concurrent INSERTs to the log.
    //   - The `CHECK (id = 0)` constraint prevents accidental
    //     multi-row inserts that would split the watermark.
    //
    // Seeded at `applied_seq = 0`, which matches the default of the
    // `session_flags.applied_seq` column (migration 65). A fresh DB
    // that has never applied a fold reads the watermark as 0 and
    // starts from `seq > 0` — i.e. the first row of the log.
    r#"BEGIN;
CREATE TABLE fold_state (
    id          INTEGER PRIMARY KEY CHECK (id = 0),
    applied_seq INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO fold_state (id, applied_seq) VALUES (0, 0);
COMMIT;"#,
];
