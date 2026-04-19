//! CQRS Phase 5 PR 5.4 — `session_flags` shadow parity comparator.
//!
//! Compares each session's legacy `sessions.{archived_at, dismissed_at,
//! category_*, classified_at}` columns against the `session_flags`
//! shadow columns materialised by the PR 5.3 fold task. Drift is
//! reported per-field with the `field` label matching the eventual
//! `shadow_flags_diff_total{field}` Prometheus counter (Phase 7 adds
//! the /metrics endpoint; until then `run_parity_sweep` emits the
//! counts at INFO level via `tracing`).
//!
//! Follows the Phase 2 comparator shape in `indexer_v2::drift` so the
//! two shadow systems stay conceptually aligned.
//!
//! ## Timestamp normalisation
//!
//! - `sessions.archived_at` / `classified_at`: RFC3339 string written
//!   with `Utc::now().to_rfc3339()` by PR 5.2 archivers / classifiers.
//! - `session_flags.archived_at` / `classified_at` / `dismissed_at`:
//!   unix ms written by the fold task from `session_action_log.at`
//!   (which is `Utc::now().timestamp_millis()`).
//! - These come from *two separate* `Utc::now()` calls inside the same
//!   TX, so they can differ by sub-millisecond. Parity compares with a
//!   ≤1s tolerance — a drift > 1 s is a real bug, not clock jitter.
//!
//! ## Null semantics
//!
//! Both sides may be NULL (e.g. an unarchived session has no
//! `archived_at` on either column). NULL == NULL is a clean match,
//! NULL != Some is a drift.
//!
//! ## Why `dismissed_at` is NOT in parity
//!
//! `sessions.dismissed_at` was DROPPED in migration 63 (CQRS Phase 0
//! IRREVERSIBLE). Dismiss lived as an in-memory `closed_ring` entry
//! until PR 5.2 introduced the action log, so `session_flags.dismissed_at`
//! is the FIRST-ever on-disk persistence of dismissals. With no legacy
//! column to compare against, parity is structurally undefined —
//! `FLAG_FIELDS` intentionally excludes it. Phase 7 may revisit if
//! dismiss needs cross-source validation (e.g. audit log replay).

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::{Database, DbResult};

/// Tolerance for RFC3339 ↔ unix-ms comparison (see module docs).
const TIMESTAMP_TOLERANCE_MS: i64 = 1_000;

/// Stable per-field diff labels. Matches the `field` label on
/// `shadow_flags_diff_total{field}` so dashboards + alerts stay
/// schema-stable as more fields fold in. `dismissed_at` is excluded
/// by design — see module docs.
pub const FLAG_FIELDS: &[&str] = &[
    "archived_at",
    "category_l1",
    "category_l2",
    "category_l3",
    "category_confidence",
    "category_source",
    "classified_at",
];

/// A single per-field drift between the legacy `sessions` row and the
/// shadow `session_flags` row.
#[derive(Debug, Clone, PartialEq)]
pub struct FlagFieldDiff {
    pub field: &'static str,
    pub legacy: String,
    pub shadow: String,
}

/// Per-session parity report. `diffs` empty = clean.
#[derive(Debug, Clone, PartialEq)]
pub struct FlagParityReport {
    pub session_id: String,
    pub diffs: Vec<FlagFieldDiff>,
}

impl FlagParityReport {
    pub fn is_clean(&self) -> bool {
        self.diffs.is_empty()
    }
}

/// Aggregated result of a parity sweep over `limit` sessions.
///
/// `per_field_counts` keys match [`FLAG_FIELDS`] — the eventual
/// `shadow_flags_diff_total{field}` Prometheus counter reads this
/// map directly.
#[derive(Debug, Clone, Default)]
pub struct ParitySweepSummary {
    pub total_sampled: u64,
    pub total_missing_shadow: u64,
    pub total_diverged: u64,
    pub per_field_counts: BTreeMap<&'static str, u64>,
}

#[derive(Debug, Default)]
struct LegacyFlags {
    archived_at_ms: Option<i64>,
    category_l1: Option<String>,
    category_l2: Option<String>,
    category_l3: Option<String>,
    category_confidence: Option<f64>,
    category_source: Option<String>,
    classified_at_ms: Option<i64>,
}

#[derive(Debug, Default)]
struct ShadowFlags {
    archived_at_ms: Option<i64>,
    category_l1: Option<String>,
    category_l2: Option<String>,
    category_l3: Option<String>,
    category_confidence: Option<f64>,
    category_source: Option<String>,
    classified_at_ms: Option<i64>,
}

fn parse_rfc3339_to_ms(s: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&Utc).timestamp_millis())
}

async fn load_legacy(db: &Database, session_id: &str) -> DbResult<Option<LegacyFlags>> {
    let row: Option<(
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT archived_at, category_l1, category_l2, category_l3,
                category_confidence, category_source, classified_at
         FROM sessions
         WHERE id = ?1",
    )
    .bind(session_id)
    .fetch_optional(db.pool())
    .await?;

    Ok(row.map(|r| LegacyFlags {
        archived_at_ms: r.0.as_deref().and_then(parse_rfc3339_to_ms),
        category_l1: r.1,
        category_l2: r.2,
        category_l3: r.3,
        category_confidence: r.4,
        category_source: r.5,
        classified_at_ms: r.6.as_deref().and_then(parse_rfc3339_to_ms),
    }))
}

async fn load_shadow(db: &Database, session_id: &str) -> DbResult<Option<ShadowFlags>> {
    let row: Option<(
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<f64>,
        Option<String>,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT archived_at, category_l1, category_l2, category_l3,
                category_confidence, category_source, classified_at
         FROM session_flags
         WHERE session_id = ?1",
    )
    .bind(session_id)
    .fetch_optional(db.pool())
    .await?;

    Ok(row.map(|r| ShadowFlags {
        archived_at_ms: r.0,
        category_l1: r.1,
        category_l2: r.2,
        category_l3: r.3,
        category_confidence: r.4,
        category_source: r.5,
        classified_at_ms: r.6,
    }))
}

fn ts_close_enough(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() <= TIMESTAMP_TOLERANCE_MS,
        _ => false,
    }
}

fn float_close_enough(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() <= 1e-6,
        _ => false,
    }
}

/// Compare legacy vs shadow flag columns for one session.
///
/// `Ok(None)` when the legacy row is missing — nothing to compare.
/// A missing *shadow* row with a populated legacy row IS drift (the
/// fold task hasn't caught up, or is stalled) and is reported as
/// diffs on every mismatched field.
pub async fn compare_flags_session(
    db: &Database,
    session_id: &str,
) -> DbResult<Option<FlagParityReport>> {
    let Some(legacy) = load_legacy(db, session_id).await? else {
        return Ok(None);
    };
    let shadow = load_shadow(db, session_id).await?.unwrap_or_default();

    let mut diffs = Vec::new();
    macro_rules! push {
        ($field:literal, $a:expr, $b:expr) => {
            diffs.push(FlagFieldDiff {
                field: $field,
                legacy: format!("{:?}", $a),
                shadow: format!("{:?}", $b),
            });
        };
    }

    if !ts_close_enough(legacy.archived_at_ms, shadow.archived_at_ms) {
        push!("archived_at", legacy.archived_at_ms, shadow.archived_at_ms);
    }
    if legacy.category_l1 != shadow.category_l1 {
        push!("category_l1", legacy.category_l1, shadow.category_l1);
    }
    if legacy.category_l2 != shadow.category_l2 {
        push!("category_l2", legacy.category_l2, shadow.category_l2);
    }
    if legacy.category_l3 != shadow.category_l3 {
        push!("category_l3", legacy.category_l3, shadow.category_l3);
    }
    if !float_close_enough(legacy.category_confidence, shadow.category_confidence) {
        push!(
            "category_confidence",
            legacy.category_confidence,
            shadow.category_confidence
        );
    }
    if legacy.category_source != shadow.category_source {
        push!(
            "category_source",
            legacy.category_source,
            shadow.category_source
        );
    }
    if !ts_close_enough(legacy.classified_at_ms, shadow.classified_at_ms) {
        push!(
            "classified_at",
            legacy.classified_at_ms,
            shadow.classified_at_ms
        );
    }

    Ok(Some(FlagParityReport {
        session_id: session_id.to_string(),
        diffs,
    }))
}

/// Walk up to `limit` recent sessions and aggregate per-field drift
/// counts. Returns a summary suitable for `tracing::info!` logging +
/// the eventual `/metrics` exporter.
///
/// Samples by most recent `last_message_at` — matches the UX priority
/// (users notice drift on fresh sessions first). Full-table passes
/// can be requested by caller with `limit = i64::MAX`.
pub async fn run_parity_sweep(db: &Database, limit: i64) -> DbResult<ParitySweepSummary> {
    let ids: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM sessions
         ORDER BY COALESCE(last_message_at, 0) DESC
         LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(db.pool())
    .await?;

    let mut summary = ParitySweepSummary {
        total_sampled: ids.len() as u64,
        ..Default::default()
    };
    for field in FLAG_FIELDS {
        summary.per_field_counts.insert(*field, 0);
    }

    for (id,) in ids {
        let Some(report) = compare_flags_session(db, &id).await? else {
            continue;
        };

        // Shadow row missing AND legacy has any populated field = drift.
        // compare_flags_session already emits diffs for each mismatched
        // field in that case, so this check is for summary-level
        // counters (session-level granularity).
        let shadow_exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM session_flags WHERE session_id = ?1")
                .bind(&id)
                .fetch_optional(db.pool())
                .await?;
        if shadow_exists.is_none() && !report.is_clean() {
            summary.total_missing_shadow += 1;
        }

        if !report.is_clean() {
            summary.total_diverged += 1;
            for diff in &report.diffs {
                *summary.per_field_counts.entry(diff.field).or_insert(0) += 1;
            }
        }
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn seed_legacy(
        db: &Database,
        id: &str,
        archived: Option<&str>,
        classified: Option<&str>,
        l1: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO sessions (id, project_id, file_path, is_sidechain,
                                  archived_at, category_l1, classified_at)
             VALUES (?1, 'p', ?2, 0, ?3, ?4, ?5)",
        )
        .bind(id)
        .bind(format!("/tmp/{id}.jsonl"))
        .bind(archived)
        .bind(l1)
        .bind(classified)
        .execute(db.pool())
        .await
        .unwrap();
    }

    async fn seed_shadow(
        db: &Database,
        id: &str,
        archived_ms: Option<i64>,
        classified_ms: Option<i64>,
        l1: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO session_flags (session_id, archived_at, category_l1, classified_at, applied_seq)
             VALUES (?1, ?2, ?3, ?4, 0)",
        )
        .bind(id)
        .bind(archived_ms)
        .bind(l1)
        .bind(classified_ms)
        .execute(db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn compare_returns_none_when_legacy_row_missing() {
        let db = Database::new_in_memory().await.unwrap();
        let report = compare_flags_session(&db, "ghost").await.unwrap();
        assert!(report.is_none());
    }

    #[tokio::test]
    async fn compare_reports_missing_shadow_as_drift_on_every_populated_field() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(
            &db,
            "no-shadow",
            Some("2026-01-01T00:00:00Z"),
            Some("2026-01-01T00:00:00Z"),
            Some("engineering"),
        )
        .await;

        let report = compare_flags_session(&db, "no-shadow")
            .await
            .unwrap()
            .unwrap();
        assert!(!report.is_clean(), "missing shadow = drift");
        let fields: Vec<&str> = report.diffs.iter().map(|d| d.field).collect();
        assert!(fields.contains(&"archived_at"));
        assert!(fields.contains(&"category_l1"));
        assert!(fields.contains(&"classified_at"));
    }

    #[tokio::test]
    async fn compare_reports_clean_when_legacy_and_shadow_match() {
        let db = Database::new_in_memory().await.unwrap();
        let rfc = "2026-01-01T00:00:00.000Z";
        let ms = parse_rfc3339_to_ms(rfc).unwrap();
        seed_legacy(&db, "match", Some(rfc), Some(rfc), Some("engineering")).await;
        seed_shadow(&db, "match", Some(ms), Some(ms), Some("engineering")).await;

        let report = compare_flags_session(&db, "match").await.unwrap().unwrap();
        assert!(
            report.is_clean(),
            "matching rows should be clean; got {:?}",
            report.diffs
        );
    }

    #[tokio::test]
    async fn compare_tolerates_sub_second_timestamp_drift() {
        // RFC3339 parse + Utc::now().timestamp_millis() can differ by
        // ~1-50 ms in prod because they come from two Utc::now() calls.
        let db = Database::new_in_memory().await.unwrap();
        let rfc = "2026-01-01T00:00:00.000Z";
        let shadow_ms = parse_rfc3339_to_ms(rfc).unwrap() + 42; // 42 ms drift
        seed_legacy(&db, "within-tol", Some(rfc), None, None).await;
        seed_shadow(&db, "within-tol", Some(shadow_ms), None, None).await;

        let report = compare_flags_session(&db, "within-tol")
            .await
            .unwrap()
            .unwrap();
        assert!(
            report.is_clean(),
            "sub-second drift is clock jitter, not drift"
        );
    }

    #[tokio::test]
    async fn compare_flags_timestamp_drift_beyond_tolerance() {
        let db = Database::new_in_memory().await.unwrap();
        let rfc = "2026-01-01T00:00:00.000Z";
        let shadow_ms = parse_rfc3339_to_ms(rfc).unwrap() + 5_000; // 5s > 1s tol
        seed_legacy(&db, "beyond-tol", Some(rfc), None, None).await;
        seed_shadow(&db, "beyond-tol", Some(shadow_ms), None, None).await;

        let report = compare_flags_session(&db, "beyond-tol")
            .await
            .unwrap()
            .unwrap();
        assert!(!report.is_clean());
        let fields: Vec<&str> = report.diffs.iter().map(|d| d.field).collect();
        assert!(fields.contains(&"archived_at"));
    }

    #[tokio::test]
    async fn compare_flags_category_string_mismatch() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "cat", None, None, Some("engineering")).await;
        seed_shadow(&db, "cat", None, None, Some("marketing")).await;

        let report = compare_flags_session(&db, "cat").await.unwrap().unwrap();
        let diff = report
            .diffs
            .iter()
            .find(|d| d.field == "category_l1")
            .unwrap();
        assert_eq!(diff.legacy, "Some(\"engineering\")");
        assert_eq!(diff.shadow, "Some(\"marketing\")");
    }

    #[tokio::test]
    async fn sweep_aggregates_per_field_counts() {
        let db = Database::new_in_memory().await.unwrap();
        // Two sessions with category drift, one clean.
        seed_legacy(&db, "a", None, None, Some("x")).await;
        seed_shadow(&db, "a", None, None, Some("y")).await;
        seed_legacy(&db, "b", None, None, Some("p")).await;
        seed_shadow(&db, "b", None, None, Some("q")).await;
        seed_legacy(&db, "c", None, None, Some("m")).await;
        seed_shadow(&db, "c", None, None, Some("m")).await;

        let summary = run_parity_sweep(&db, 100).await.unwrap();
        assert_eq!(summary.total_sampled, 3);
        assert_eq!(summary.total_diverged, 2);
        assert_eq!(summary.per_field_counts["category_l1"], 2);
        assert_eq!(summary.per_field_counts["archived_at"], 0);
    }

    #[tokio::test]
    async fn sweep_returns_zero_counts_when_all_match() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "clean", None, None, Some("x")).await;
        seed_shadow(&db, "clean", None, None, Some("x")).await;

        let summary = run_parity_sweep(&db, 100).await.unwrap();
        assert_eq!(summary.total_diverged, 0);
        for &field in FLAG_FIELDS {
            assert_eq!(
                summary.per_field_counts[field], 0,
                "{field} should have 0 drift count"
            );
        }
    }

    #[tokio::test]
    async fn sweep_counts_missing_shadow_rows() {
        let db = Database::new_in_memory().await.unwrap();
        seed_legacy(&db, "lonely", Some("2026-01-01T00:00:00Z"), None, None).await;
        // No shadow row

        let summary = run_parity_sweep(&db, 100).await.unwrap();
        assert_eq!(summary.total_sampled, 1);
        assert_eq!(summary.total_diverged, 1);
        assert_eq!(summary.total_missing_shadow, 1);
        assert_eq!(summary.per_field_counts["archived_at"], 1);
    }
}
