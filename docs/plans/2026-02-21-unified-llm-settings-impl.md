# Unified LLM Settings — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all hardcoded `ClaudeCliProvider::new("haiku")` calls with a unified settings system persisted in SQLite, wire the existing ProviderSettings UI, and surface generation model + tokens in report details.

**Architecture:** Single-row `app_settings` DB table stores user's model preference. A shared `create_llm_provider()` factory reads settings and constructs the provider. All LLM callsites (classify + reports) use this factory. CLI response metadata (model, tokens) is captured instead of discarded and surfaced in the report details UI.

**Tech Stack:** Rust (sqlx, axum, serde, ts-rs), React, TypeScript, Tailwind CSS

---

### Task 1: Add `app_settings` DB table (Backend — db)

**Files:**
- Modify: `crates/db/src/migrations.rs:603` (add migration 27)

**Step 1: Write the failing test**

Add to `crates/db/src/migrations.rs` in the `mod tests` block (after the existing migration tests):

```rust
#[tokio::test]
async fn test_migration27_app_settings_table() {
    let pool = setup_db().await;

    // Table should exist with exactly one default row
    let row: (String, i64) = sqlx::query_as(
        "SELECT llm_model, llm_timeout_secs FROM app_settings WHERE id = 1"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(row.0, "haiku");
    assert_eq!(row.1, 120);

    // CHECK constraint: inserting id != 1 should fail
    let result = sqlx::query("INSERT INTO app_settings (id, llm_model) VALUES (2, 'sonnet')")
        .execute(&pool)
        .await;
    assert!(result.is_err());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_migration27`
Expected: FAIL — table `app_settings` does not exist

**Step 3: Write minimal implementation**

Add **one** multi-statement migration entry to the `MIGRATIONS` array in `crates/db/src/migrations.rs` (after the last entry, before the closing `];`). This uses `BEGIN;`/`COMMIT;` so it occupies a single version slot. Note: the comment label "Migration 27" is a feature-group name — the runtime version number will be the array index + 1 (currently ~97), not 27:

```rust
    // Migration 27: Unified LLM settings (replaces hardcoded "haiku" in classify + reports).
    // Default timeout is 120s (matches the existing report generation timeout, which is the
    // longest LLM operation). Classification also uses this timeout — 120s is fine for classify
    // since it only affects the max wait, not the typical latency.
    r#"BEGIN;
CREATE TABLE IF NOT EXISTS app_settings (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    llm_model        TEXT NOT NULL DEFAULT 'haiku',
    llm_timeout_secs INTEGER NOT NULL DEFAULT 120
);
INSERT OR IGNORE INTO app_settings (id) VALUES (1);
COMMIT;"#,
```

> **Why multi-statement?** The migration system already supports `BEGIN;`/`COMMIT;` blocks via `raw_sql()` (see `lib.rs:184-193`). Using a single array entry keeps comment-to-version alignment: "Migration 27" = version 27. This is safe here because both `CREATE TABLE IF NOT EXISTS` and `INSERT OR IGNORE` are fully idempotent — unlike `ALTER TABLE ADD COLUMN` which fails on re-run (see Task 6 for why ALTERs use separate entries).

**Step 4: Run test to verify it passes**

Run: `cargo test -p claude-view-db test_migration27`
Expected: PASS

**Step 5: Commit**

```
feat(db): add app_settings table for unified LLM config (migration 27)
```

---

### Task 2: Add `get/update_app_settings` DB queries (Backend — db)

**Files:**
- Create: `crates/db/src/queries/settings.rs`
- Modify: `crates/db/src/queries/mod.rs` (add `pub mod settings;` after `pub mod reports;`)
- Modify: `crates/db/src/lib.rs` (add re-export after the `pub use queries::reports::{...}` line)

**Step 1: Write the failing test**

Create `crates/db/src/queries/settings.rs`:

```rust
//! App settings CRUD queries.

use crate::{Database, DbResult};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Application settings (single-row table).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub llm_model: String,
    #[ts(type = "number")]
    pub llm_timeout_secs: i64,
}

impl Database {
    /// Read current app settings.
    pub async fn get_app_settings(&self) -> DbResult<AppSettings> {
        todo!()
    }

    /// Update app settings (partial — only provided fields are changed).
    pub async fn update_app_settings(&self, model: Option<&str>, timeout_secs: Option<i64>) -> DbResult<AppSettings> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    #[tokio::test]
    async fn test_get_default_settings() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.get_app_settings().await.unwrap();
        assert_eq!(settings.llm_model, "haiku");
        assert_eq!(settings.llm_timeout_secs, 120);
    }

    #[tokio::test]
    async fn test_update_model() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(Some("sonnet"), None).await.unwrap();
        assert_eq!(settings.llm_model, "sonnet");
        assert_eq!(settings.llm_timeout_secs, 120); // unchanged

        let read_back = db.get_app_settings().await.unwrap();
        assert_eq!(read_back.llm_model, "sonnet");
    }

    #[tokio::test]
    async fn test_update_timeout() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(None, Some(90)).await.unwrap();
        assert_eq!(settings.llm_model, "haiku"); // unchanged
        assert_eq!(settings.llm_timeout_secs, 90);
    }

    #[tokio::test]
    async fn test_update_both() {
        let db = Database::new_in_memory().await.unwrap();
        let settings = db.update_app_settings(Some("opus"), Some(180)).await.unwrap();
        assert_eq!(settings.llm_model, "opus");
        assert_eq!(settings.llm_timeout_secs, 180);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-db test_get_default_settings test_update_model test_update_timeout test_update_both`
Expected: FAIL — `todo!()` panics

**Step 3: Write minimal implementation**

Replace the `todo!()` bodies in `settings.rs`:

```rust
impl Database {
    pub async fn get_app_settings(&self) -> DbResult<AppSettings> {
        let row: (String, i64) = sqlx::query_as(
            "SELECT llm_model, llm_timeout_secs FROM app_settings WHERE id = 1"
        )
        .fetch_one(self.pool())
        .await?;
        Ok(AppSettings {
            llm_model: row.0,
            llm_timeout_secs: row.1,
        })
    }

    pub async fn update_app_settings(
        &self,
        model: Option<&str>,
        timeout_secs: Option<i64>,
    ) -> DbResult<AppSettings> {
        if let Some(m) = model {
            sqlx::query("UPDATE app_settings SET llm_model = ? WHERE id = 1")
                .bind(m)
                .execute(self.pool())
                .await?;
        }
        if let Some(t) = timeout_secs {
            sqlx::query("UPDATE app_settings SET llm_timeout_secs = ? WHERE id = 1")
                .bind(t)
                .execute(self.pool())
                .await?;
        }
        self.get_app_settings().await
    }
}
```

Then wire the module:

In `crates/db/src/queries/mod.rs`, add after `pub mod reports;`:
```rust
pub mod settings;
```

In `crates/db/src/lib.rs`, add after the `pub use queries::reports::{...}` line:
```rust
pub use queries::settings::AppSettings;
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-db settings`
Expected: ALL PASS

**Step 5: Commit**

```
feat(db): add get/update_app_settings queries
```

---

### Task 3: Add `GET/PUT /api/settings` routes (Backend — server)

**Files:**
- Create: `crates/server/src/routes/settings.rs`
- Modify: `crates/server/src/routes/mod.rs:115` (register settings router)

**Step 1: Create the settings route module**

Create `crates/server/src/routes/settings.rs`:

```rust
//! App settings API routes.

use std::sync::Arc;
use axum::{extract::State, routing::{get, put}, Json, Router};
use serde::Deserialize;

use crate::{error::ApiError, state::AppState};
use claude_view_db::AppSettings;

/// Allowed model values for Claude CLI.
const VALID_MODELS: &[&str] = &["haiku", "sonnet", "opus"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateSettingsRequest {
    llm_model: Option<String>,
    llm_timeout_secs: Option<i64>,
}

async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AppSettings>, ApiError> {
    let settings = state.db.get_app_settings().await
        .map_err(|e| ApiError::Internal(format!("Failed to read settings: {e}")))?;
    Ok(Json(settings))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateSettingsRequest>,
) -> Result<Json<AppSettings>, ApiError> {
    // Validate model if provided
    if let Some(ref m) = body.llm_model {
        if !VALID_MODELS.contains(&m.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "Invalid model '{}'. Valid options: {}",
                m,
                VALID_MODELS.join(", ")
            )));
        }
    }

    // Validate timeout if provided
    if let Some(t) = body.llm_timeout_secs {
        if t < 10 || t > 300 {
            return Err(ApiError::BadRequest(
                "Timeout must be between 10 and 300 seconds".to_string()
            ));
        }
    }

    let settings = state.db.update_app_settings(
        body.llm_model.as_deref(),
        body.llm_timeout_secs,
    ).await.map_err(|e| ApiError::Internal(format!("Failed to update settings: {e}")))?;

    Ok(Json(settings))
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/settings", get(get_settings).put(update_settings))
}
```

**Step 2: Register the router**

In `crates/server/src/routes/mod.rs`, add `pub mod settings;` to the module declarations (between `pub mod score;` and `pub mod search;` alphabetically), and add this line after the `.nest("/api", reports::router())` line:

```rust
        .nest("/api", settings::router())
```

**Step 3: Run the full server test suite**

Run: `cargo test -p claude-view-server`
Expected: PASS (compiles and existing tests still pass)

**Step 4: Commit**

```
feat(api): add GET/PUT /api/settings endpoints
```

---

### Task 4: Add `model` field to `CompletionResponse` and parse CLI metadata (Backend — core)

**Files:**
- Modify: `crates/core/src/llm/types.rs:46-51` (add `model` field)
- Modify: `crates/core/src/llm/claude_cli.rs:326-340` (parse model + tokens from CLI JSON)

> **CLI JSON format (verified empirically):** `claude -p "..." --output-format json --model haiku` returns:
> ```json
> {
>   "type": "result", "subtype": "success",
>   "result": "...",
>   "usage": { "input_tokens": 10, "output_tokens": 321, ... },
>   "modelUsage": { "claude-haiku-4-5-20251001": { "inputTokens": 10, "outputTokens": 321, "costUSD": 0.006 } },
>   "total_cost_usd": 0.006163
> }
> ```
> Key: there is **no top-level `model` field**. Model ID is the first key of `modelUsage`. Tokens are in `usage.input_tokens`/`usage.output_tokens`.

**Step 1: Write the failing test**

First, add a test that constructs a `CompletionResponse` with a `model` field — this will fail to compile before Step 3 adds the field. Add to `crates/core/src/llm/types.rs` tests:

```rust
#[test]
fn test_completion_response_has_model_field() {
    let resp = CompletionResponse {
        content: "hello".to_string(),
        model: Some("claude-haiku-4-5-20251001".to_string()),
        input_tokens: Some(10),
        output_tokens: Some(321),
        latency_ms: 100,
    };
    assert_eq!(resp.model.as_deref(), Some("claude-haiku-4-5-20251001"));
}
```

Run: `cargo test -p claude-view-core test_completion_response_has_model`
Expected: FAIL — `CompletionResponse` has no field named `model`

**Step 2: Add `model` field to `CompletionResponse`**

In `crates/core/src/llm/types.rs`, add the `model` field (line 47, between `content` and `input_tokens`):

```rust
/// Response from a general-purpose LLM completion.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub model: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub latency_ms: u64,
}
```

**Step 3: Fix all existing `CompletionResponse {` constructions**

Search for `CompletionResponse {` and add `model: None` where needed. There is currently one site at `claude_cli.rs:335`.

**Step 4: Run Step 1 test again**

Run: `cargo test -p claude-view-core test_completion_response_has_model`
Expected: PASS

**Step 5: Write the CLI JSON parsing test**

Add to `crates/core/src/llm/claude_cli.rs` tests. This tests against the **actual** CLI JSON format:

```rust
#[test]
fn test_parse_cli_json_response_extracts_metadata() {
    // Actual Claude CLI --output-format json response shape (verified 2026-02-21)
    let cli_json = serde_json::json!({
        "type": "result",
        "subtype": "success",
        "result": "Here is the report content...",
        "usage": {
            "input_tokens": 1200,
            "output_tokens": 340,
            "cache_creation_input_tokens": 0,
            "cache_read_input_tokens": 45000
        },
        "modelUsage": {
            "claude-haiku-4-5-20251001": {
                "inputTokens": 1200,
                "outputTokens": 340,
                "costUSD": 0.006163
            }
        },
        "total_cost_usd": 0.006163
    });

    // Extract content
    let content = cli_json["result"]
        .as_str()
        .unwrap_or("")
        .to_string();
    assert_eq!(content, "Here is the report content...");

    // Extract model from modelUsage keys (first key = model ID)
    let model = cli_json["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next().cloned());
    assert_eq!(model.as_deref(), Some("claude-haiku-4-5-20251001"));

    // Extract tokens from usage (snake_case fields)
    let input_tokens = cli_json["usage"]["input_tokens"].as_u64();
    let output_tokens = cli_json["usage"]["output_tokens"].as_u64();
    assert_eq!(input_tokens, Some(1200));
    assert_eq!(output_tokens, Some(340));
}

#[test]
fn test_parse_cli_json_missing_model_usage_returns_none() {
    // Graceful fallback when modelUsage is absent
    let cli_json = serde_json::json!({
        "type": "result",
        "result": "content"
    });

    let model = cli_json["modelUsage"]
        .as_object()
        .and_then(|m| m.keys().next().cloned());
    assert!(model.is_none());

    let input_tokens = cli_json["usage"]["input_tokens"].as_u64();
    assert!(input_tokens.is_none());
}
```

Run: `cargo test -p claude-view-core test_parse_cli_json`
Expected: PASS (these test the parsing logic we'll inline next)

**Step 6: Update `complete()` to parse metadata from CLI JSON**

In `crates/core/src/llm/claude_cli.rs`, replace lines 326-340 (inside the `complete` method):

```rust
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| LlmError::ParseFailed(format!("Invalid JSON from CLI: {e}")))?;

        let content = parsed["result"]
            .as_str()
            .unwrap_or_else(|| parsed["content"].as_str().unwrap_or(""))
            .to_string();

        // Model ID is the first key of the "modelUsage" object (no top-level "model" field).
        let model = parsed["modelUsage"]
            .as_object()
            .and_then(|m| m.keys().next().cloned());

        let input_tokens = parsed["usage"]["input_tokens"].as_u64();
        let output_tokens = parsed["usage"]["output_tokens"].as_u64();

        Ok(CompletionResponse {
            content,
            model,
            input_tokens,
            output_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
        })
```

**Step 7: Run full core tests**

Run: `cargo test -p claude-view-core`
Expected: ALL PASS

**Step 8: Commit**

```
feat(core): parse model + tokens from Claude CLI JSON response
```

---

### Task 5: Create shared provider factory and replace hardcoded callsites (Backend — server)

**Files:**
- Modify: `crates/server/src/routes/classify.rs:222-223,481-482,627-628`
- Modify: `crates/server/src/routes/reports.rs:167`

**Step 1: Add the shared helper**

Add to `crates/server/src/routes/settings.rs` (public, so other route modules can use it):

```rust
/// Create an LLM provider from the user's persisted settings.
///
/// All LLM callsites MUST use this instead of hardcoding a model.
pub async fn create_llm_provider(
    db: &claude_view_db::Database,
) -> Result<claude_view_core::llm::ClaudeCliProvider, ApiError> {
    let settings = db.get_app_settings().await
        .map_err(|e| ApiError::Internal(format!("Failed to read LLM settings: {e}")))?;
    Ok(claude_view_core::llm::ClaudeCliProvider::new(&settings.llm_model)
        .with_timeout(settings.llm_timeout_secs.clamp(10, 300) as u64))
}
```

**Step 2: Replace classify.rs hardcoded providers**

In `crates/server/src/routes/classify.rs`:

At line 222-223, replace:
```rust
    let provider_name = "claude-cli";
    let model_name = "haiku";
```
with:
```rust
    let settings = state.db.get_app_settings().await.map_err(|e| {
        ApiError::Internal(format!("Failed to read LLM settings: {e}"))
    })?;
    let provider_name = "claude-cli";
    let model_name = settings.llm_model.clone();
```

At line 481-482, replace:
```rust
    let provider =
        claude_view_core::llm::ClaudeCliProvider::new("haiku").with_timeout(60);
```
with:
```rust
    let provider = super::settings::create_llm_provider(&state.db).await?;
```

At line 627-628 (inside `run_classification`, within the `for input in batch` loop), replace:
```rust
            let single_provider =
                claude_view_core::llm::ClaudeCliProvider::new("haiku").with_timeout(60);
```
with (`db` is already bound at line 541 as `let db = &state.db;`):
```rust
            let single_provider = match super::settings::create_llm_provider(db).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create LLM provider");
                    continue;
                }
            };
```

Note for line 627-628: This is inside a loop. The provider reads settings once per session classified. This is fine — SQLite reads are <1ms and it respects mid-batch setting changes.

**Step 3: Replace reports.rs hardcoded provider**

In `crates/server/src/routes/reports.rs`, at line 167, replace:
```rust
    let provider = claude_view_core::llm::ClaudeCliProvider::new("haiku").with_timeout(120);
```
with:
```rust
    let provider = super::settings::create_llm_provider(&state.db).await.map_err(|e| {
        GENERATING.store(false, Ordering::SeqCst);
        e
    })?;
```

**Step 4: Run full test suite**

Run: `cargo test -p claude-view-server && cargo test -p claude-view-core`
Expected: ALL PASS

**Step 5: Verify no remaining hardcoded providers**

Search: `grep -r 'ClaudeCliProvider::new("haiku")' crates/server/`
Expected: Zero matches

**Step 6: Commit**

```
refactor: replace all hardcoded ClaudeCliProvider::new("haiku") with settings factory
```

---

### Task 6: Add generation metadata to reports table and API (Backend — db + server)

**Files:**
- Modify: `crates/db/src/migrations.rs` (migration 28: add columns)
- Modify: `crates/db/src/queries/reports.rs:11-30` (ReportRow struct)
- Modify: `crates/db/src/queries/reports.rs:60-91` (insert_report)
- Modify: `crates/db/src/queries/reports.rs:94-107` (list_reports)
- Modify: `crates/db/src/queries/reports.rs:110-121` (get_report)

**Step 1: Add migrations 28-30**

In `crates/db/src/migrations.rs`, add after the migration 27 entry. Use **separate entries** (one ALTER per version slot) so each is independently protected by the "duplicate column name" error handler:

```rust
    // Migrations 28-30: Report generation metadata (model + token counts).
    // Separate entries so each ALTER is independently idempotent — if one column
    // already exists (branch collision), the others still get created.
    r#"ALTER TABLE reports ADD COLUMN generation_model TEXT;"#,
    r#"ALTER TABLE reports ADD COLUMN generation_input_tokens INTEGER;"#,
    r#"ALTER TABLE reports ADD COLUMN generation_output_tokens INTEGER;"#,
```

> **Why NOT multi-statement here?** Unlike Task 1's `CREATE TABLE IF NOT EXISTS` (which is inherently idempotent), `ALTER TABLE ADD COLUMN` fails with "duplicate column name" on re-run. In a `BEGIN;`/`COMMIT;` block, the first failing ALTER would abort the entire transaction, preventing subsequent ALTERs from running. Separate entries ensure each ALTER is independently protected by the error handler at `lib.rs:196`.

**Step 2: Add fields to ReportRow struct**

In `crates/db/src/queries/reports.rs`, add after `generation_ms` field (line 28):

```rust
    pub generation_model: Option<String>,
    #[ts(type = "number | null")]
    pub generation_input_tokens: Option<i64>,
    #[ts(type = "number | null")]
    pub generation_output_tokens: Option<i64>,
```

**Step 3: Update `insert_report` to accept new params**

Replace the `insert_report` function signature and body to add three new parameters:

```rust
    pub async fn insert_report(
        &self,
        report_type: &str,
        date_start: &str,
        date_end: &str,
        content_md: &str,
        context_digest: Option<&str>,
        session_count: i64,
        project_count: i64,
        total_duration_secs: i64,
        total_cost_cents: i64,
        generation_ms: Option<i64>,
        generation_model: Option<&str>,
        generation_input_tokens: Option<i64>,
        generation_output_tokens: Option<i64>,
    ) -> DbResult<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"INSERT INTO reports (report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
               RETURNING id"#,
        )
        .bind(report_type)
        .bind(date_start)
        .bind(date_end)
        .bind(content_md)
        .bind(context_digest)
        .bind(session_count)
        .bind(project_count)
        .bind(total_duration_secs)
        .bind(total_cost_cents)
        .bind(generation_ms)
        .bind(generation_model)
        .bind(generation_input_tokens)
        .bind(generation_output_tokens)
        .fetch_one(self.pool())
        .await?;
        Ok(row.0)
    }
```

**Step 4: Update `list_reports` and `get_report` SELECT queries**

Replace the entire `list_reports` method body (lines 94-107) with:

```rust
    pub async fn list_reports(&self) -> DbResult<Vec<ReportRow>> {
        let rows = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, i64, i64, i64, i64, Option<i64>, Option<String>, Option<i64>, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at FROM reports ORDER BY created_at DESC, id DESC"
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at)| ReportRow {
                id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at,
            })
            .collect())
    }
```

Replace the entire `get_report` method body (lines 110-121) with:

```rust
    pub async fn get_report(&self, id: i64) -> DbResult<Option<ReportRow>> {
        let row = sqlx::query_as::<_, (i64, String, String, String, String, Option<String>, i64, i64, i64, i64, Option<i64>, Option<String>, Option<i64>, Option<i64>, String)>(
            "SELECT id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at FROM reports WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|(id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at)| ReportRow {
            id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, generation_model, generation_input_tokens, generation_output_tokens, created_at,
        }))
    }
```

**IMPORTANT:** The 3 new columns (`generation_model`, `generation_input_tokens`, `generation_output_tokens`) are inserted BEFORE `created_at` in the tuple — matching the order they appear in `ReportRow` struct after the changes in Step 2. Column order in the tuple must match the SELECT list exactly.

**Step 5: Update existing tests**

All `insert_report` test calls now need 3 extra `None` arguments for the new params:

```rust
// Before:
db.insert_report("daily", "2026-02-21", "2026-02-21", "content", None, 8, 3, 15120, 680, Some(14200))

// After:
db.insert_report("daily", "2026-02-21", "2026-02-21", "content", None, 8, 3, 15120, 680, Some(14200), None, None, None)
```

**Also update `insert_report` calls in server tests** (`crates/server/src/routes/reports.rs`). There are 3 test calls at lines 433, 464, and 513 that also need the extra `None, None, None` arguments:

```rust
// Line 433: test_list_reports_with_data
db.insert_report("daily", "2026-02-21", "2026-02-21", "- Did stuff", None, 5, 2, 3600, 100, None, None, None, None)

// Line 464: test_get_report_by_id
db.insert_report("weekly", "2026-02-17", "2026-02-21", "week summary", None, 32, 5, 64800, 2450, None, None, None, None)

// Line 513: test_delete_report
db.insert_report("daily", "2026-02-21", "2026-02-21", "test", None, 1, 1, 100, 10, None, None, None, None)
```

Add a new test for generation metadata:

```rust
#[tokio::test]
async fn test_insert_report_with_generation_metadata() {
    let db = Database::new_in_memory().await.unwrap();
    let id = db.insert_report(
        "daily", "2026-02-21", "2026-02-21", "content", None,
        8, 3, 15120, 680, Some(14200),
        Some("claude-haiku-4-5-20251001"), Some(1200), Some(340),
    ).await.unwrap();

    let report = db.get_report(id).await.unwrap().unwrap();
    assert_eq!(report.generation_model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert_eq!(report.generation_input_tokens, Some(1200));
    assert_eq!(report.generation_output_tokens, Some(340));
}
```

**Step 6: Run tests**

Run: `cargo test -p claude-view-db`
Expected: ALL PASS

**Step 7: Regenerate TS types**

Run: `cargo test -p claude-view-db export_bindings` (or however ts-rs export is triggered)

Verify `src/types/generated/ReportRow.ts` now includes `generationModel`, `generationInputTokens`, `generationOutputTokens`.

**IMPORTANT (full-stack wiring check):** After regeneration, run `bun run build` (or `tsc --noEmit`) to confirm the frontend compiles with the updated types. Per CLAUDE.md: `Option`/`undefined` silently absorbs gaps — if the TS type export failed, the frontend would compile silently but new fields would be `undefined` everywhere. A build check catches this before proceeding to Tasks 8-9.

**Step 8: Commit**

```
feat(db): add generation_model + token columns to reports table (migrations 28-30)
```

---

### Task 7: Wire generation metadata through report SSE stream (Backend — server)

**Files:**
- Modify: `crates/server/src/routes/reports.rs:167-224` (capture model from provider, pass to insert + SSE)

**Step 1: Capture model name from provider**

In `crates/server/src/routes/reports.rs`, after creating the provider (Task 5 already changed line 167), capture the model name before moving into the stream:

```rust
    let provider = super::settings::create_llm_provider(&state.db).await.map_err(|e| {
        GENERATING.store(false, Ordering::SeqCst);
        e
    })?;
    let generation_model = provider.model().to_string();
```

`generation_model` will be implicitly captured by the `async_stream::stream!` macro (same as `report_type`, `date_start`, etc. at lines 180-183). No explicit `move` keyword is needed — `async_stream::stream!` automatically moves all referenced variables. Just reference `generation_model` inside the block (in Step 2 and Step 3 below) and the capture happens automatically.

**Step 2: Update `insert_report` call**

Replace the insert_report call (around line 204) to pass generation metadata:

```rust
        let report_id = db.insert_report(
            &report_type,
            &date_start,
            &date_end,
            &full_content,
            context_digest_json.as_deref(),
            preview.session_count,
            preview.project_count,
            preview.total_duration_secs,
            preview.total_cost_cents,
            Some(generation_ms),
            Some(&generation_model),
            None, // input_tokens — stream_completion doesn't return usage
            None, // output_tokens — stream_completion doesn't return usage
        ).await;
```

Note: `stream_completion` streams text line by line and doesn't get usage stats back from the CLI (unlike `--output-format json`). Token counts will be `None` for streaming reports. This is acceptable — the model name is the important piece.

**Step 3: Add model to SSE `done` event**

Update the `done_json` (around line 219):

```rust
                let done_json = serde_json::json!({
                    "reportId": id,
                    "generationMs": generation_ms,
                    "generationModel": &generation_model,
                    "contextDigest": context_digest_json,
                });
```

**Step 4: Run tests**

Run: `cargo test -p claude-view-server`
Expected: ALL PASS

**Step 5: Commit**

```
feat(api): include generation model in report SSE done event
```

---

### Task 8: Wire ProviderSettings UI to API (Frontend)

**Files:**
- Modify: `src/components/ProviderSettings.tsx:67-69,172-192`
- Create: `src/hooks/use-app-settings.ts`

**Step 1: Create the settings hook**

Create `src/hooks/use-app-settings.ts` (uses `@tanstack/react-query`, matching project conventions — see `use-reports.ts` for the pattern):

```typescript
import { useQuery, useQueryClient } from '@tanstack/react-query'

interface AppSettings {
  llmModel: string
  llmTimeoutSecs: number
}

async function fetchSettings(): Promise<AppSettings> {
  const res = await fetch('/api/settings')
  if (!res.ok) throw new Error(`Failed to fetch settings: ${await res.text()}`)
  return res.json()
}

export function useAppSettings() {
  const queryClient = useQueryClient()

  const { data, error, isLoading } = useQuery({
    queryKey: ['app-settings'],
    queryFn: fetchSettings,
    staleTime: 30_000,
  })

  const updateSettings = async (updates: Partial<AppSettings>) => {
    const res = await fetch('/api/settings', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(updates),
    })
    if (!res.ok) throw new Error(await res.text())
    const updated: AppSettings = await res.json()
    queryClient.setQueryData(['app-settings'], updated)
    return updated
  }

  return { settings: data, error, isLoading, updateSettings }
}
```

**Step 2: Update ProviderSettings to use the hook**

In `src/components/ProviderSettings.tsx`:

Replace the local state (lines 67-69):
```typescript
const [selectedProvider, setSelectedProvider] = useState<ProviderType>('claude-cli')
const [model, setModel] = useState('haiku')
```

With:
```typescript
const { settings, updateSettings } = useAppSettings()
const [selectedProvider, setSelectedProvider] = useState<ProviderType>('claude-cli')
const model = settings?.llmModel ?? 'haiku'

const handleModelChange = async (newModel: string) => {
  await updateSettings({ llmModel: newModel })
}
```

Update the model `<select>` onChange handler (line 182):
```typescript
onChange={(e) => handleModelChange(e.target.value)}
```

**Step 3: Update the header text**

Change the header (line 112) from:
```
Classification Provider
```
to:
```
AI Provider
```

Update the description (line 123) from:
```
Choose how sessions are classified. The provider determines which LLM service is used for categorization.
```
to:
```
Choose the LLM model for AI features (classification, report generation). Applies to all features.
```

**Step 4: Verify in browser**

Open the app, go to Settings/System page where ProviderSettings lives. Change the model dropdown. Refresh page — should persist.

**Step 5: Commit**

```
feat(ui): wire ProviderSettings to /api/settings endpoint
```

---

### Task 9: Show generation model in ReportDetails UI (Frontend)

**Files:**
- Modify: `src/hooks/use-report-generate.ts:12-18,86-92` (capture model from done event)
- Modify: `src/components/reports/ReportDetails.tsx:36-39,66-73` (add model prop + display)
- Modify: `src/components/reports/ReportCard.tsx:91-94,141-144` (pass model through)

**Step 1: Update the generate hook to capture model**

In `src/hooks/use-report-generate.ts`:

Add state for generation model (after line 24):
```typescript
const [generationModel, setGenerationModel] = useState<string | null>(null)
```

In the `done` event handler (line 86-92), add:
```typescript
if (parsed.generationModel) {
  setGenerationModel(parsed.generationModel)
}
```

Add `setGenerationModel(null)` to the reset function (line 30):
```typescript
const reset = useCallback(() => {
  setStreamedText('')
  setContextDigest(null)
  setGenerationModel(null)
  setError(null)
  setIsGenerating(false)
}, [])
```

Add `generationModel` to the return value (line 112):
```typescript
return { generate, isGenerating, streamedText, contextDigest, generationModel, error, reset }
```

Update the `UseReportGenerateReturn` interface to include `generationModel: string | null`.

**Step 2: Update ReportDetails props**

In `src/components/reports/ReportDetails.tsx`, update the props interface (lines 36-39):

```typescript
interface ReportDetailsProps {
  contextDigestJson: string | null
  totalCostCents: number
  generationModel?: string | null
  generationInputTokens?: number | null
  generationOutputTokens?: number | null
}
```

**Step 3: Add generation metadata display**

In `ReportDetails.tsx`, add a generation info line in the collapsed state (after the existing cost/tokens preview, around line 70):

```typescript
{!expanded && props.generationModel && (
  <span className="ml-1 text-gray-300 dark:text-gray-600">
    · Generated by {formatModelName(props.generationModel)}
  </span>
)}
```

And in the expanded panel, add a row before the cost row (before line 83):

```typescript
{props.generationModel && (
  <div className="flex flex-wrap gap-x-3 gap-y-1 text-gray-600 dark:text-gray-400">
    <span>Generated by <span className="text-gray-900 dark:text-gray-200 font-medium">{formatModelName(props.generationModel)}</span></span>
    {props.generationInputTokens != null && props.generationInputTokens > 0 && (
      <span>({formatTokens(props.generationInputTokens + (props.generationOutputTokens ?? 0))} tokens)</span>
    )}
  </div>
)}
```

Import `formatModelName` at the top of the file:
```typescript
import { formatModelName } from '../../lib/format-model'
```

**Step 4: Pass model through ReportCard**

In `src/components/reports/ReportCard.tsx`:

First, update the destructuring at line 34 to include `generationModel`:
```typescript
const { generate, isGenerating, streamedText, contextDigest: completedContextDigest, error, reset, generationModel } = useReportGenerate()
```

Then update both places where `<ReportDetails>` is rendered:

For the COMPLETE state (around line 91-94):
```typescript
<ReportDetails
  contextDigestJson={completedContextDigest ?? existingReport?.contextDigest ?? null}
  totalCostCents={existingReport?.totalCostCents ?? 0}
  generationModel={generationModel ?? existingReport?.generationModel}
  generationInputTokens={existingReport?.generationInputTokens}
  generationOutputTokens={existingReport?.generationOutputTokens}
/>
```

For the EXISTING state (around line 141-144):
```typescript
<ReportDetails
  contextDigestJson={existingReport.contextDigest ?? null}
  totalCostCents={existingReport.totalCostCents}
  generationModel={existingReport.generationModel}
  generationInputTokens={existingReport.generationInputTokens}
  generationOutputTokens={existingReport.generationOutputTokens}
/>
```

**Step 5: Verify in browser**

Generate a new report. Expand the details section. Should show "Generated by Claude Haiku 4.5" (or whatever model was used).

**Step 6: Commit**

```
feat(ui): show generation model in report details
```

---

### Task 10: End-to-end verification

**Step 1: Run full backend test suite**

Run: `cargo test`
Expected: ALL PASS

**Step 2: Run full frontend test suite**

Run: `bun test`
Expected: ALL PASS (or only pre-existing failures)

**Step 3: Manual verification**

1. Start the app: `bun run dev:all`
2. Go to System/Settings page → change model to "sonnet"
3. Refresh → confirm it persists as "sonnet"
4. Go to Reports page → generate a report
5. Expand Details → should show "Generated by Claude Sonnet 4.5" (or current version)
6. Go to Sessions → classify a session → should use Sonnet (check server logs)
7. Change model back to "haiku" → classify another → should use Haiku

**Step 4: Verify no hardcoded providers remain**

Run: `grep -r 'new("haiku")' crates/server/`
Expected: Zero matches

**Step 5: Final commit if any cleanup needed**

```
chore: unified LLM settings end-to-end verification complete
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Task 5 Site C: `db_clone` variable doesn't exist in `run_classification` — would fail to compile | Blocker | Changed `&db_clone` → `&db` (bound at line 541 as `let db = &state.db;`) |
| 2 | Task 8: `useSWR` from `swr` used but `swr` is not installed — project uses `@tanstack/react-query` | Blocker | Rewrote `use-app-settings.ts` hook using `useQuery`/`useQueryClient` matching project conventions |
| 3 | Task 3: Plan said `mod settings;` (private) but route modules need `pub mod` to expose `router()` | Warning | Changed to `pub mod settings;` with alphabetical placement guidance |
| 4 | Task 2: Hardcoded line numbers (`:16`, `:21`) are fragile and off-by-one | Warning | Replaced with content-based anchors ("after `pub mod reports;`", "after the reports re-export line") |
| 5 | Task 1: Plan implied "Migration 27 = version 27" but runtime version is array-index-based (~97) | Warning | Added clarifying note that comment label ≠ runtime version |
| 6 | Task 3: Router registration line reference hardcoded to `:115` | Minor | Changed to content-based anchor ("after the reports router line") |
| 7 | Task 5 Site C: Missing context about which function and variable scope | Minor | Added note that this is inside `run_classification` and `db` is from line 541 |
| 8 | Task 6 Step 4: `list_reports`/`get_report` had only prose, no code — implementer would have to guess 15-element tuple ordering | Blocker | Added complete verbatim code for both methods with correct column ordering |
| 9 | Task 6: `insert_report` calls in `crates/server/src/routes/reports.rs` tests (lines 433, 464, 513) would fail to compile — plan only mentioned db tests | Blocker | Added explicit list of 3 server test call sites that need `None, None, None` appended |
| 10 | Task 7: "Move `generation_model` into the stream closure" was vague prose — unclear for someone unfamiliar with `async_stream` | Blocker | Added explicit explanation that `async_stream::stream!` auto-captures referenced variables |
| 11 | Task 5: `settings.llm_timeout_secs as u64` could wrap negative `i64` to enormous `u64` if DB is corrupted | Important | Added `.clamp(10, 300)` before the `as u64` cast for defense-in-depth |
