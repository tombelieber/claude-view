# ReportDetails Expandable Raw Stats — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the expandable raw stats section showing cost, tokens, tools, skills, and per-project breakdowns in completed reports.

**Architecture:** Add `context_digest` + token fields to the backend data path (ContextDigest struct → DB → API → TS type), then build a new `ReportDetails` React component that parses the JSON and renders a collapsible details panel. Use `ui-ux-pro-max` skill for the component.

**Tech Stack:** Rust (serde, sqlx, ts-rs), React, TypeScript, Tailwind CSS, lucide-react

---

### Task 1: Add token fields to ContextDigest struct (Backend — core)

**Files:**
- Modify: `crates/core/src/report.rs:32-39` (ContextDigest struct)
- Modify: `crates/core/src/report.rs:153-198` (sample_digest in tests)

**Step 1: Write the failing test**

Add to `crates/core/src/report.rs` in the `mod tests` block:

```rust
#[test]
fn test_context_digest_token_serialization() {
    let digest = ContextDigest {
        total_input_tokens: 847_000,
        total_output_tokens: 124_000,
        ..sample_digest()
    };
    let json = serde_json::to_string(&digest).unwrap();
    assert!(json.contains("\"total_input_tokens\":847000"));
    assert!(json.contains("\"total_output_tokens\":124000"));

    // Round-trip
    let parsed: ContextDigest = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total_input_tokens, 847_000);
    assert_eq!(parsed.total_output_tokens, 124_000);
}

#[test]
fn test_context_digest_defaults_tokens_to_zero() {
    let digest = ContextDigest::default();
    assert_eq!(digest.total_input_tokens, 0);
    assert_eq!(digest.total_output_tokens, 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-core test_context_digest_token`
Expected: FAIL — `ContextDigest` has no field `total_input_tokens`

**Step 3: Write minimal implementation**

Add two fields to the `ContextDigest` struct at `crates/core/src/report.rs:32-39`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextDigest {
    pub report_type: String,
    pub date_range: String,
    pub projects: Vec<ProjectDigest>,
    pub top_tools: Vec<String>,
    pub top_skills: Vec<String>,
    pub summary_line: String,
    #[serde(default)]
    pub total_input_tokens: u64,
    #[serde(default)]
    pub total_output_tokens: u64,
}
```

`#[serde(default)]` ensures backward compatibility: existing JSON without these fields deserializes to 0.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core`
Expected: ALL PASS (existing tests still work since Default provides 0)

**Step 5: Commit**

```bash
git add crates/core/src/report.rs
git commit -m "feat(core): add token fields to ContextDigest"
```

---

### Task 2: Add token aggregation to DB query (Backend — db)

**Files:**
- Modify: `crates/db/src/queries/reports.rs:131-149` (get_sessions_in_range)

**Step 1: Write the failing test**

The existing `get_sessions_in_range` returns a 6-tuple. We need to add `total_input_tokens` and `total_output_tokens` making it an 8-tuple. First update the return type, which will break the existing caller.

Actually — cleaner approach: add a **new** dedicated query rather than modifying the existing tuple. Add to `crates/db/src/queries/reports.rs`:

```rust
/// Query aggregate token totals for sessions in a date range.
/// Returns (total_input_tokens, total_output_tokens).
pub async fn get_token_totals_in_range(
    &self,
    start_ts: i64,
    end_ts: i64,
) -> DbResult<(i64, i64)> {
    let row: (i64, i64) = sqlx::query_as(
        r#"SELECT
            COALESCE(SUM(total_input_tokens), 0),
            COALESCE(SUM(total_output_tokens), 0)
           FROM sessions
           WHERE first_message_at >= ? AND first_message_at <= ?"#,
    )
    .bind(start_ts)
    .bind(end_ts)
    .fetch_one(self.pool())
    .await?;
    Ok(row)
}
```

Test (add to `mod tests` in the same file):

```rust
#[tokio::test]
async fn test_get_token_totals_in_range() {
    let db = Database::new_in_memory().await.unwrap();

    // Insert sessions with token data
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, duration_seconds, total_input_tokens, total_output_tokens) VALUES ('s1', 'p1', '/tmp/s1.jsonl', 'Test', 'proj', 1000, 100, 500000, 80000)"
    ).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO sessions (id, project_id, file_path, preview, project_display_name, first_message_at, duration_seconds, total_input_tokens, total_output_tokens) VALUES ('s2', 'p1', '/tmp/s2.jsonl', 'Test', 'proj', 1100, 200, 347000, 44000)"
    ).execute(db.pool()).await.unwrap();

    let (input, output) = db.get_token_totals_in_range(0, 2000).await.unwrap();
    assert_eq!(input, 847000);
    assert_eq!(output, 124000);
}

#[tokio::test]
async fn test_get_token_totals_empty_range() {
    let db = Database::new_in_memory().await.unwrap();
    let (input, output) = db.get_token_totals_in_range(0, 2000).await.unwrap();
    assert_eq!(input, 0);
    assert_eq!(output, 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_get_token_totals`
Expected: FAIL — method doesn't exist

**Step 3: Implement the method**

Add the `get_token_totals_in_range` method shown above to the `impl Database` block in `crates/db/src/queries/reports.rs`, after `get_top_skills_in_range`.

**Step 4: Run tests**

Run: `cargo test -p claude-view-db`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add crates/db/src/queries/reports.rs
git commit -m "feat(db): add get_token_totals_in_range query"
```

---

### Task 3: Wire token totals into build_context_digest (Backend — server)

**Files:**
- Modify: `crates/server/src/routes/reports.rs:238-330` (build_context_digest fn)

**Step 1: Add token query call to build_context_digest**

In `crates/server/src/routes/reports.rs`, inside `build_context_digest()`, after the `top_skills` query (line ~287), add:

```rust
// Query token totals
let (total_input_tokens, total_output_tokens) = state.db
    .get_token_totals_in_range(start_ts, end_ts)
    .await
    .unwrap_or((0, 0));
```

Then in the `ContextDigest` construction (line ~322), add the fields:

```rust
Ok(ContextDigest {
    report_type: report_type.to_string(),
    date_range,
    projects,
    top_tools,
    top_skills,
    summary_line: format!("{total_sessions} sessions across {total_projects} projects"),
    total_input_tokens: total_input_tokens as u64,
    total_output_tokens: total_output_tokens as u64,
})
```

**Step 2: Run tests**

Run: `cargo test -p claude-view-server`
Expected: ALL PASS (existing tests compile — they don't test token values but the struct now has them)

**Step 3: Commit**

```bash
git add crates/server/src/routes/reports.rs
git commit -m "feat(server): populate token totals in context digest"
```

---

### Task 4: Expose context_digest in ReportRow API (Backend — db)

**Critical gap:** The `ReportRow` struct does NOT include `context_digest`. It's stored in the DB but never returned to the frontend. Without this, the frontend has nothing to parse.

**Files:**
- Modify: `crates/db/src/queries/reports.rs:8-29` (ReportRow struct)
- Modify: `crates/db/src/queries/reports.rs:93-106` (list_reports query)
- Modify: `crates/db/src/queries/reports.rs:109-120` (get_report query)

**Step 1: Write the failing test**

Add to `mod tests` in `crates/db/src/queries/reports.rs`:

```rust
#[tokio::test]
async fn test_report_includes_context_digest() {
    let db = Database::new_in_memory().await.unwrap();
    let digest_json = r#"{"report_type":"daily","date_range":"2026-02-21","projects":[],"top_tools":["Read"],"top_skills":[],"summary_line":"test","total_input_tokens":847000,"total_output_tokens":124000}"#;
    let id = db
        .insert_report("daily", "2026-02-21", "2026-02-21", "test", Some(digest_json), 1, 1, 100, 10, None)
        .await
        .unwrap();

    let report = db.get_report(id).await.unwrap().unwrap();
    assert_eq!(report.context_digest.as_deref(), Some(digest_json));

    let list = db.list_reports().await.unwrap();
    assert_eq!(list[0].context_digest.as_deref(), Some(digest_json));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p claude-view-db test_report_includes_context_digest`
Expected: FAIL — `ReportRow` has no field `context_digest`

**Step 3: Implement**

3a. Add field to `ReportRow` struct:

```rust
pub struct ReportRow {
    #[ts(type = "number")]
    pub id: i64,
    pub report_type: String,
    pub date_start: String,
    pub date_end: String,
    pub content_md: String,
    pub context_digest: Option<String>,   // <-- NEW
    #[ts(type = "number")]
    pub session_count: i64,
    // ... rest unchanged
}
```

3b. Update `list_reports` SELECT to include `context_digest`:

Change the query to:
```sql
SELECT id, report_type, date_start, date_end, content_md, context_digest, session_count, project_count, total_duration_secs, total_cost_cents, generation_ms, created_at FROM reports ORDER BY created_at DESC, id DESC
```

Update the tuple type to `(i64, String, String, String, String, Option<String>, i64, i64, i64, i64, Option<i64>, String)` and the map closure to include `context_digest`.

3c. Update `get_report` SELECT similarly.

**Step 4: Run tests**

Run: `cargo test -p claude-view-db`
Expected: ALL PASS

**Step 5: Regenerate TypeScript types**

Run: `cargo test -p claude-view-db export_bindings` (or just `cargo test -p claude-view-db` — ts-rs exports on test)

Verify `src/types/generated/ReportRow.ts` now includes `contextDigest: string | null`.

**Step 6: Commit**

```bash
git add crates/db/src/queries/reports.rs src/types/generated/ReportRow.ts
git commit -m "feat(db): expose context_digest in ReportRow API response"
```

---

### Task 5: Build ReportDetails component (Frontend)

**REQUIRED:** Use `ui-ux-pro-max` skill for this component.

**Files:**
- Create: `src/components/reports/ReportDetails.tsx`

**Step 1: Define the ContextDigest type**

At the top of `ReportDetails.tsx`, define the interface (matches Rust `ContextDigest` struct):

```typescript
interface ContextDigest {
  report_type: string
  date_range: string
  summary_line: string
  total_input_tokens?: number
  total_output_tokens?: number
  projects: {
    name: string
    session_count: number
    commit_count: number
    total_duration_secs: number
    branches: {
      name: string
      sessions: { first_prompt: string; category: string | null; duration_secs: number }[]
    }[]
  }[]
  top_tools: string[]
  top_skills: string[]
}
```

**Step 2: Implement formatting helpers**

```typescript
function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}
```

**Step 3: Implement the component**

```typescript
interface ReportDetailsProps {
  contextDigestJson: string | null
  totalCostCents: number
}

export function ReportDetails({ contextDigestJson, totalCostCents }: ReportDetailsProps) {
  const [expanded, setExpanded] = useState(false)

  // Parse context_digest JSON — hide section entirely if unparseable
  const digest = useMemo(() => {
    if (!contextDigestJson) return null
    try {
      return JSON.parse(contextDigestJson) as ContextDigest
    } catch {
      return null
    }
  }, [contextDigestJson])

  if (!digest) return null

  // ... render collapsed/expanded UI with chevron toggle,
  //     cost line, token line, tools, skills, per-project breakdown
}
```

**Key UI elements (use ui-ux-pro-max for design):**
- Collapsed: `[ChevronRight] Details` button, subtle text
- Expanded: `[ChevronDown] Details` button + content panel
- Content:
  - Line 1: `Cost: $X.XX · Tokens: XK in / XK out`
  - Line 2: `Top tools: Read · Edit · Bash` (from `top_tools` array)
  - Line 3: `Top skills: /commit · /review-pr` (from `top_skills` array, if non-empty)
  - Per-project: `name — N sessions · Xh Ym · N commits`
  - Under project: `└ branch1 (N) · branch2 (N)`
- Use `overflow-hidden` + `max-h-0` → `max-h-96` transition for smooth expand
- Tailwind + `dark:` variants only (no shadcn CSS vars)
- lucide-react for chevron icon

**Step 4: Verify TypeScript compilation**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 5: Commit**

```bash
git add src/components/reports/ReportDetails.tsx
git commit -m "feat(ui): add ReportDetails expandable raw stats component"
```

---

### Task 6: Wire ReportDetails into ReportCard (Frontend)

**Files:**
- Modify: `src/components/reports/ReportCard.tsx`

**Step 1: Import and wire**

Add import at top:
```typescript
import { ReportDetails } from './ReportDetails'
```

**Step 2: Add to COMPLETE state (freshly generated report)**

In the COMPLETE state block (lines 73-92), the current report was just streamed — we don't have `contextDigest` yet because it's from the stream, not the DB row. However, once the SSE "done" event fires, the report is persisted. We need the context_digest from the persisted report.

**Solution:** The SSE `done` event returns `reportId`. After generation completes, the `useReportGenerate` hook should fetch the persisted report to get `contextDigest`. OR simpler: pass `contextDigest` through the SSE done event.

**Simpler approach:** The frontend doesn't need context_digest for freshly streamed reports during the same session (user can expand history to see details). Instead, wire `ReportDetails` into:
1. The **existing report view** (line 96-116) where `existingReport` has the full `ReportRow` including `contextDigest`
2. The **ReportHistory selected view** (if that becomes a thing)

For the COMPLETE state (just-streamed), we can't show details immediately without an extra fetch. Accept this limitation — the user can click the report in history to see details. OR: add `contextDigest` to the `done` SSE event payload.

**Recommended:** Add `contextDigest` to the SSE done event. It's already serialized in the server. Then pass it to ReportDetails.

In `crates/server/src/routes/reports.rs`, SSE done event (line ~219), add:
```rust
let done_json = serde_json::json!({
    "reportId": id,
    "generationMs": generation_ms,
    "contextDigest": context_digest_json,
});
```

In `src/hooks/use-report-generate.ts`, parse the new field from the done event and expose it.

Then in `ReportCard.tsx`:

In the COMPLETE state block, after `<ReportContent>`:
```tsx
<ReportDetails
  contextDigestJson={completedContextDigest ?? existingReport?.contextDigest ?? null}
  totalCostCents={existingReport?.totalCostCents ?? 0}
/>
```

In the existing report view block, after `<ReportContent>`:
```tsx
<ReportDetails
  contextDigestJson={existingReport.contextDigest ?? null}
  totalCostCents={existingReport.totalCostCents}
/>
```

**Step 3: Update use-report-generate hook**

In `src/hooks/use-report-generate.ts`, extract `contextDigest` from the SSE done event and expose it:
- Add `contextDigest` state
- Parse from done event data: `const { contextDigest } = JSON.parse(event.data)`
- Return it from the hook

**Step 4: Verify**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 5: Commit**

```bash
git add crates/server/src/routes/reports.rs src/hooks/use-report-generate.ts src/components/reports/ReportCard.tsx
git commit -m "feat: wire ReportDetails into ReportCard for all report views"
```

---

### Task 7: Full verification

**Step 1: Run all backend tests**

Run: `cargo test -p claude-view-core -p claude-view-db -p claude-view-server`
Expected: ALL PASS

**Step 2: Run frontend type check**

Run: `bunx tsc --noEmit`
Expected: exit 0

**Step 3: Verify TypeScript type file**

Read `src/types/generated/ReportRow.ts` and confirm `contextDigest: string | null` is present.

**Step 4: Commit any final fixes**

Only if needed.

---

## Dependency Graph

```
Task 1 (ContextDigest tokens) ──┐
                                 ├── Task 3 (wire into build_context_digest)
Task 2 (DB token query)  ───────┘         │
                                           ├── Task 6 (wire into ReportCard)
Task 4 (ReportRow context_digest) ─────────┤
                                           │
Task 5 (ReportDetails component) ──────────┘
                                           │
                                     Task 7 (verify)
```

Tasks 1, 2, 4 can run in parallel (no deps between them).
Task 3 depends on 1 + 2.
Task 5 can run in parallel with 1-4 (pure frontend, uses TS type manually).
Task 6 depends on 3 + 4 + 5.
Task 7 depends on 6.
