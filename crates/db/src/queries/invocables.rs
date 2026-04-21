// crates/db/src/queries/invocables.rs
// Invocable + Invocation CRUD operations.

use std::collections::HashMap;

use super::invocation_agg::{
    aggregate_all, classify_key, display_name, load_invocation_totals, ToolKind,
};
use super::row_types::batch_insert_invocations_tx;
use super::{InvocableWithCount, StatsOverview};
use crate::{Database, DbResult};

/// Best-effort heuristic that maps a JSON `invocation_counts` key to the
/// `invocables.id` used by the classifier. Used to attach registry
/// metadata (plugin_name, description) to aggregated counts. Returns the
/// raw key when nothing better can be inferred.
fn key_to_invocable_id(key: &str) -> String {
    match classify_key(key) {
        ToolKind::Tool => format!("builtin:{key}"),
        ToolKind::Agent => format!("builtin:{}", display_name(key)),
        ToolKind::Skill | ToolKind::McpTool => display_name(key).to_string(),
    }
}

impl Database {
    /// Insert or update a single invocable.
    ///
    /// Uses `INSERT ... ON CONFLICT(id) DO UPDATE SET` to upsert.
    pub async fn upsert_invocable(
        &self,
        id: &str,
        plugin_name: Option<&str>,
        name: &str,
        kind: &str,
        description: &str,
    ) -> DbResult<()> {
        sqlx::query(
            r#"
            INSERT INTO invocables (id, plugin_name, name, kind, description, status)
            VALUES (?1, ?2, ?3, ?4, ?5, 'enabled')
            ON CONFLICT(id) DO UPDATE SET
                plugin_name = excluded.plugin_name,
                name = excluded.name,
                kind = excluded.kind,
                description = excluded.description
            "#,
        )
        .bind(id)
        .bind(plugin_name)
        .bind(name)
        .bind(kind)
        .bind(description)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    /// Batch insert invocations in a single transaction.
    ///
    /// Each tuple is `(source_file, byte_offset, invocable_id, session_id, project, timestamp)`.
    /// Uses `INSERT OR IGNORE` so re-indexing skips duplicates (PK is source_file + byte_offset).
    /// Returns the number of rows actually inserted.
    pub async fn batch_insert_invocations(
        &self,
        invocations: &[(String, i64, String, String, String, i64)],
    ) -> DbResult<u64> {
        if invocations.is_empty() {
            return Ok(0);
        }
        let mut tx = self.pool().begin().await?;
        let inserted = batch_insert_invocations_tx(&mut tx, invocations).await?;
        tx.commit().await?;
        Ok(inserted)
    }

    /// List all invocables with their invocation counts.
    ///
    /// Counts are aggregated from `session_stats.invocation_counts` (JSON
    /// per session) and mapped back onto rows from the `invocables`
    /// registry via a best-effort key-to-id heuristic. Invocables that
    /// have never been used simply get `invocation_count = 0`.
    ///
    /// Results are ordered by `invocation_count DESC`, then `name ASC`.
    /// `last_used_at` is no longer available in the CQRS path — the
    /// column is left as `None` rather than fabricated.
    pub async fn list_invocables_with_counts(&self) -> DbResult<Vec<InvocableWithCount>> {
        let registry_rows: Vec<(String, Option<String>, String, String, String)> =
            sqlx::query_as(r#"SELECT id, plugin_name, name, kind, description FROM invocables"#)
                .fetch_all(self.pool())
                .await?;

        let totals = load_invocation_totals(self.pool()).await?;
        let mut by_id: HashMap<String, i64> = HashMap::new();
        for (key, count) in &totals {
            *by_id.entry(key_to_invocable_id(key)).or_default() += *count;
        }

        let mut out: Vec<InvocableWithCount> = registry_rows
            .into_iter()
            .map(|(id, plugin_name, name, kind, description)| {
                let invocation_count = by_id.remove(&id).unwrap_or(0);
                InvocableWithCount {
                    id,
                    plugin_name,
                    name,
                    kind,
                    description,
                    invocation_count,
                    last_used_at: None,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.invocation_count
                .cmp(&a.invocation_count)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    }

    /// Batch insert/update invocables from a registry snapshot.
    ///
    /// Each tuple is `(id, plugin_name, name, kind, description)`.
    /// Uses `INSERT ... ON CONFLICT(id) DO UPDATE SET` for upsert semantics.
    /// Returns the number of rows affected.
    pub async fn batch_upsert_invocables(
        &self,
        invocables: &[(String, Option<String>, String, String, String)],
    ) -> DbResult<u64> {
        let mut tx = self.pool().begin().await?;
        let mut affected: u64 = 0;

        for (id, plugin_name, name, kind, description) in invocables {
            let result = sqlx::query(
                r#"
                INSERT INTO invocables (id, plugin_name, name, kind, description, status)
                VALUES (?1, ?2, ?3, ?4, ?5, 'enabled')
                ON CONFLICT(id) DO UPDATE SET
                    plugin_name = excluded.plugin_name,
                    name = excluded.name,
                    kind = excluded.kind,
                    description = excluded.description
                "#,
            )
            .bind(id)
            .bind(plugin_name)
            .bind(name)
            .bind(kind)
            .bind(description)
            .execute(&mut *tx)
            .await?;

            affected += result.rows_affected();
        }

        tx.commit().await?;
        Ok(affected)
    }

    /// Get aggregate statistics overview.
    ///
    /// `total_invocations` and `unique_invocables_used` are derived from
    /// `session_stats.invocation_counts` — the CQRS Phase 6 replacement
    /// for the legacy `invocations` table scan.
    pub async fn get_stats_overview(&self) -> DbResult<StatsOverview> {
        let (total_sessions,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM valid_sessions")
            .fetch_one(self.pool())
            .await?;

        let summary = aggregate_all(self.pool()).await?;

        let all = self.list_invocables_with_counts().await?;
        let top_invocables: Vec<InvocableWithCount> = all.into_iter().take(10).collect();

        Ok(StatsOverview {
            total_sessions,
            total_invocations: summary.total_invocations,
            unique_invocables_used: summary.unique_invocables,
            top_invocables,
        })
    }
}
