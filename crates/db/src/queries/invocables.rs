// crates/db/src/queries/invocables.rs
// Invocable + Invocation CRUD operations.

use crate::{Database, DbResult};
use super::row_types::batch_insert_invocations_tx;
use super::{InvocableWithCount, StatsOverview};

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
    /// Results are ordered by invocation_count DESC, then name ASC.
    pub async fn list_invocables_with_counts(&self) -> DbResult<Vec<InvocableWithCount>> {
        let rows: Vec<InvocableWithCount> = sqlx::query_as(
            r#"
            SELECT
                i.id, i.plugin_name, i.name, i.kind, i.description,
                COALESCE(COUNT(inv.invocable_id), 0) as invocation_count,
                MAX(inv.timestamp) as last_used_at
            FROM invocables i
            LEFT JOIN invocations inv ON i.id = inv.invocable_id
            GROUP BY i.id
            ORDER BY invocation_count DESC, i.name ASC
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows)
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
    /// Returns total sessions, total invocations, unique invocables used,
    /// and the top 10 invocables by usage count.
    pub async fn get_stats_overview(&self) -> DbResult<StatsOverview> {
        let (total_sessions,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions")
                .fetch_one(self.pool())
                .await?;

        let (total_invocations,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM invocations")
                .fetch_one(self.pool())
                .await?;

        let (unique_invocables_used,): (i64,) =
            sqlx::query_as("SELECT COUNT(DISTINCT invocable_id) FROM invocations")
                .fetch_one(self.pool())
                .await?;

        let all = self.list_invocables_with_counts().await?;
        let top_invocables: Vec<InvocableWithCount> = all.into_iter().take(10).collect();

        Ok(StatsOverview {
            total_sessions,
            total_invocations,
            unique_invocables_used,
            top_invocables,
        })
    }
}
