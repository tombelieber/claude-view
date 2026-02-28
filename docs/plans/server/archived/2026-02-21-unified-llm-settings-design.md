# Unified LLM Settings — Design

**Date:** 2026-02-21
**Status:** Approved

## Problem

4 places in the backend hardcode `ClaudeCliProvider::new("haiku")`:
- `classify.rs:222,481,628` (3x for classification)
- `reports.rs:167` (1x for report generation)

`ProviderSettings.tsx` has a model dropdown UI but saves to nothing. `LlmConfig` struct exists but is unused. Report generation discards CLI response metadata (model name, token counts). Report details UI cannot show what model generated the report or what it cost.

## Scope

- **In scope:** Unified settings table, API endpoints, provider factory, wire ProviderSettings UI, capture CLI response metadata, surface model + tokens in report details
- **Out of scope:** BYOK (Anthropic API, OpenAI, Ollama) — deferred to future stage
- **Default:** Claude CLI with `haiku` model

## Design

### 1. DB — `app_settings` table

New migration:

```sql
CREATE TABLE IF NOT EXISTS app_settings (
  id               INTEGER PRIMARY KEY CHECK (id = 1),
  llm_model        TEXT NOT NULL DEFAULT 'haiku',
  llm_timeout_secs INTEGER NOT NULL DEFAULT 60
);
INSERT OR IGNORE INTO app_settings (id) VALUES (1);
```

- `CHECK (id = 1)` enforces single row (standard pattern for local-first SQLite apps)
- Only Claude CLI fields for now. BYOK adds `llm_provider`, `api_key`, `endpoint` columns later
- Queries: `get_app_settings() -> AppSettings`, `update_app_settings(model, timeout)`

### 2. Backend API — `GET/PUT /api/settings`

```
GET  /api/settings  → { "llmModel": "haiku", "llmTimeoutSecs": 60 }
PUT  /api/settings  ← { "llmModel": "sonnet" }  → 200 OK
```

Validation: reject unknown model values. Accept `haiku`, `sonnet`, `opus` (Claude CLI aliases).

### 3. Provider Factory

Shared helper replaces all 4 hardcoded sites:

```rust
async fn create_llm_provider(db: &Database) -> Result<ClaudeCliProvider, ApiError> {
    let settings = db.get_app_settings().await?;
    Ok(ClaudeCliProvider::new(&settings.llm_model)
        .with_timeout(settings.llm_timeout_secs))
}
```

Callsites replaced:
- `classify.rs:222` (job creation metadata)
- `classify.rs:481` (single session classification)
- `classify.rs:628` (bulk classification loop)
- `reports.rs:167` (report generation)

### 4. CLI Response Metadata

Claude CLI `--output-format json` returns:

```json
{
  "result": "...",
  "model": "claude-haiku-4-5-20251001",
  "usage": { "input_tokens": 1200, "output_tokens": 340 }
}
```

Currently `CompletionResponse` discards model and tokens. Changes:

- Add `model: Option<String>` to `CompletionResponse`
- Parse `model`, `input_tokens`, `output_tokens` from CLI JSON response (both `spawn_and_parse` and `complete` paths)

### 5. Report Generation Metadata

**DB:** Add columns to `reports` table (new migration):

```sql
ALTER TABLE reports ADD COLUMN generation_model TEXT;
ALTER TABLE reports ADD COLUMN generation_input_tokens INTEGER;
ALTER TABLE reports ADD COLUMN generation_output_tokens INTEGER;
```

**Backend:**
- `insert_report()`: accept and store generation model + token counts
- SSE `done` event: include `"model": "claude-haiku-4-5-20251001"`, `"inputTokens"`, `"outputTokens"`

**Frontend:**
- `ReportRow` ts-rs type: add `generationModel`, `generationInputTokens`, `generationOutputTokens`
- `use-report-generate` hook: parse model + tokens from `done` event
- `ReportDetails` component: show generation metadata line using `formatModelName()` from `src/lib/format-model.ts`

### 6. Wire ProviderSettings.tsx

- Load current model from `GET /api/settings` on mount
- Save on change via `PUT /api/settings`
- Rename header from "Classification Provider" to "AI Provider"
- Keep 3 options: Haiku (default), Sonnet, Opus
- Keep "Coming Soon" on Anthropic API / OpenAI Compatible

## Data Flow

```
User picks model (ProviderSettings UI)
  → PUT /api/settings { llmModel: "sonnet" }
  → DB app_settings row updated

Any feature needing LLM:
  → create_llm_provider(db) reads app_settings
  → ClaudeCliProvider::new("sonnet").with_timeout(60)
  → claude -p --model sonnet "prompt"
  → CLI returns { model: "claude-sonnet-4-5-...", usage: {...} }
  → Metadata captured in CompletionResponse
  → Stored in reports table / classification_jobs
  → Surfaced in UI
```
