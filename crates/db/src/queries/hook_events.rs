// crates/db/src/queries/hook_events.rs
//! Hook event persistence: batch insert on SessionEnd, query for historical view.

use crate::Database;
use sqlx::Row;

/// A single hook event row for insert/select.
#[derive(Debug, Clone)]
pub struct HookEventRow {
    pub timestamp: i64,
    pub event_name: String,
    pub tool_name: Option<String>,
    pub label: String,
    pub group_name: String,
    pub context: Option<String>,
}

impl<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> for HookEventRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            timestamp: row.try_get("timestamp")?,
            event_name: row.try_get("event_name")?,
            tool_name: row.try_get("tool_name")?,
            label: row.try_get("label")?,
            group_name: row.try_get("group_name")?,
            context: row.try_get("context")?,
        })
    }
}

/// Insert hook events in a batch transaction.
///
/// Per CLAUDE.md: batch writes in transactions, never individual statements in loops.
/// This uses a single transaction for all events, committing atomically.
pub async fn insert_hook_events(
    db: &Database,
    session_id: &str,
    events: &[HookEventRow],
) -> Result<(), sqlx::Error> {
    if events.is_empty() {
        return Ok(());
    }

    let mut tx = db.pool().begin().await?;
    for event in events {
        sqlx::query(
            "INSERT INTO hook_events (session_id, timestamp, event_name, tool_name, label, group_name, context)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(event.timestamp)
        .bind(&event.event_name)
        .bind(&event.tool_name)
        .bind(&event.label)
        .bind(&event.group_name)
        .bind(&event.context)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fetch hook events for a session, ordered by timestamp ascending.
pub async fn get_hook_events(
    db: &Database,
    session_id: &str,
) -> Result<Vec<HookEventRow>, sqlx::Error> {
    let rows: Vec<HookEventRow> = sqlx::query_as(
        "SELECT timestamp, event_name, tool_name, label, group_name, context
         FROM hook_events
         WHERE session_id = ?
         ORDER BY timestamp ASC, id ASC",
    )
    .bind(session_id)
    .fetch_all(db.pool())
    .await?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_insert_and_get_hook_events() {
        let db = Database::new_in_memory().await.unwrap();

        let events = vec![
            HookEventRow {
                timestamp: 1000,
                event_name: "SessionStart".into(),
                tool_name: None,
                label: "Waiting for first prompt".into(),
                group_name: "needs_you".into(),
                context: None,
            },
            HookEventRow {
                timestamp: 1001,
                event_name: "PreToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Running: git status".into(),
                group_name: "autonomous".into(),
                context: Some(r#"{"command":"git status"}"#.into()),
            },
            HookEventRow {
                timestamp: 1002,
                event_name: "PostToolUse".into(),
                tool_name: Some("Bash".into()),
                label: "Thinking...".into(),
                group_name: "autonomous".into(),
                context: None,
            },
        ];

        insert_hook_events(&db, "test-session", &events)
            .await
            .unwrap();

        let fetched = get_hook_events(&db, "test-session").await.unwrap();
        assert_eq!(fetched.len(), 3);
        assert_eq!(fetched[0].event_name, "SessionStart");
        assert_eq!(fetched[1].event_name, "PreToolUse");
        assert_eq!(fetched[1].tool_name, Some("Bash".into()));
        assert_eq!(
            fetched[1].context,
            Some(r#"{"command":"git status"}"#.into())
        );
        assert_eq!(fetched[2].event_name, "PostToolUse");
    }

    #[tokio::test]
    async fn test_insert_empty_events() {
        let db = Database::new_in_memory().await.unwrap();
        // Should not error on empty vec
        insert_hook_events(&db, "test-session", &[]).await.unwrap();

        let fetched = get_hook_events(&db, "test-session").await.unwrap();
        assert!(fetched.is_empty());
    }

    #[tokio::test]
    async fn test_get_hook_events_nonexistent_session() {
        let db = Database::new_in_memory().await.unwrap();
        let fetched = get_hook_events(&db, "nonexistent").await.unwrap();
        assert!(fetched.is_empty());
    }
}
