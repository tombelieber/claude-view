# Work Reports — Design Doc

> **Status:** approved
> **Date:** 2026-02-21
> **Approach:** Streaming AI report generation via Claude CLI (Approach B)

## Summary

Generate ultra-lean daily/weekly work reports from Claude Code session data. The user clicks one button, Claude CLI summarizes their activity into 5-8 value-focused bullets, streamed live via SSE. Reports persist for browsing and export.

## Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Generation | Claude CLI (free, zero-config) | Reuses existing pattern, no API key needed |
| Delivery | SSE streaming | 15s wait feels instant when text streams word-by-word |
| Output style | Ultra lean, value-focused | 5-8 bullets highlighting what was shipped/fixed/designed |
| Audience | Self + manager/team | Two-layer: AI bullets (shareable) + raw stats (personal) |
| Trigger | Manual click (MVP) | Auto-generate deferred to follow-up |
| UX model | Dedicated Reports page with time-aware cards | Zero-decision: page shows most useful report to generate |
| Export | Markdown (copy + download) | Paste into Slack/Notion/email |

---

## 1. Data Flow

```
User clicks "Generate" on a card
    |
    v
POST /api/reports/generate { reportType, dateStart, dateEnd }
    |
    v
Backend queries DB --> Assembles context digest
    |                  (sessions, projects, commits, costs, tools)
    v
Spawns Claude CLI with summarization prompt on stdin
    |
    v
Pipes stdout line-by-line via tokio BufReader
    |
    v
tokio::sync::broadcast channel --> SSE handler
    |
    v
Frontend receives "chunk" events --> renders markdown live
    |
    v
On EOF: persist full content to DB --> emit "done" event with reportId
```

---

## 2. UI/UX Design

### Page Layout — Time-Aware Dual Cards

The page always leads with the most useful report to generate. No dropdowns, no toggles.

```
Morning (< noon, today has < 2 sessions):
  Primary:   Yesterday (or Last Week on Mondays)
  Secondary: Today

Afternoon/Evening:
  Primary:   Today
  Secondary: This Week
```

### Card States

**State 1: PREVIEW** (before generate)
```
+----------------------------------------------+
|  Today -- Feb 21                             |
|  8 sessions . 3 projects . 4h 12m . $6.80   |
|                                              |
|           [* Generate Report]                |
+----------------------------------------------+
```

**State 2: STREAMING** (during generate)
```
+----------------------------------------------+
|  Today -- Feb 21                             |
|                                              |
|  . Shipped full-text search_                 |
|                                              |
|           [o Generating...]                  |
+----------------------------------------------+
```

**State 3: COMPLETE** (report ready)
```
+----------------------------------------------+
|  Today -- Feb 21                             |
|                                              |
|  . Shipped full-text search (Tantivy + UI)   |
|  . Fixed SSE reconnect + ranking bugs        |
|  . Migrated vicky-wiki auth to new layout    |
|  . 8 sessions, 3 projects, 4h 12m           |
|                                              |
|  [Copy] [Export .md] [Redo]                  |
|                                              |
|  Details >                                   |
|  ............................................|
|  Cost: $6.80 . Tokens: 847K in / 124K out    |
|  Top tools: Edit(47) Read(89) Bash(23)       |
|  claude-view: 5 sessions (search, SSE, docs) |
|  vicky-wiki: 2 sessions (auth migration)     |
+----------------------------------------------+
```

**State 4: EMPTY** (no sessions in range)
```
+----------------------------------------------+
|  Today -- Feb 21                             |
|                                              |
|  No sessions yet today.                      |
|  Yesterday had 8 sessions across 3 projects. |
|                                              |
|  [* Generate Yesterday's Report]             |
+----------------------------------------------+
```

### Two-Layer Output

- **Top section** = AI-generated lean bullets. This is what Copy/Export captures. Shareable with manager.
- **Details section** = raw stats from DB (no AI). Expandable, collapsed by default. Personal reference.

Copy button only copies the top section -- clean for Slack/email.

### Report History

Below the two active cards. Simple list, newest first. Click loads that report into the main display.

### Custom Range

Small `+ Custom range` link below the cards. Opens inline date picker. For the 1% case.

---

## 3. Backend Architecture

### DB Schema

New migration:

```sql
CREATE TABLE reports (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    report_type         TEXT NOT NULL,        -- 'daily' | 'weekly' | 'custom'
    date_start          TEXT NOT NULL,        -- '2026-02-21'
    date_end            TEXT NOT NULL,        -- '2026-02-21' (same for daily)
    content_md          TEXT NOT NULL,        -- the generated markdown
    context_digest      TEXT,                 -- input sent to Claude (for redo/debug)
    session_count       INTEGER NOT NULL,
    project_count       INTEGER NOT NULL,
    total_duration_secs INTEGER NOT NULL,
    total_cost_cents    INTEGER NOT NULL,
    generation_ms       INTEGER,             -- how long CLI took
    created_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_reports_date ON reports(date_start, date_end);
CREATE INDEX idx_reports_type ON reports(report_type);
```

No foreign keys to sessions -- the report is a snapshot. If sessions get re-indexed, old reports stay intact.

### API Routes

New module: `crates/server/src/routes/reports.rs`

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/api/reports/generate` | Start generation, returns SSE stream |
| `GET` | `/api/reports` | List all reports (newest first) |
| `GET` | `/api/reports/:id` | Get single report |
| `DELETE` | `/api/reports/:id` | Delete a report |
| `GET` | `/api/reports/preview` | Context preview (stats) for a date range, no CLI call |

**POST /api/reports/generate** request:
```json
{
  "reportType": "daily",
  "dateStart": "2026-02-21",
  "dateEnd": "2026-02-21"
}
```

Response: SSE stream.
```
event: chunk
data: {"text": "* Shipped full-text "}

event: chunk
data: {"text": "search for claude-view"}

event: done
data: {"reportId": 42, "generationMs": 14200}

event: error
data: {"message": "Claude CLI not found"}
```

**GET /api/reports/preview** query params: `?type=daily` or `?dateStart=...&dateEnd=...`

Returns:
```json
{
  "sessionCount": 8,
  "projectCount": 3,
  "totalDurationSecs": 15120,
  "totalCostCents": 680,
  "projects": [
    { "name": "claude-view", "sessionCount": 5 },
    { "name": "vicky-wiki", "sessionCount": 2 },
    { "name": "dotfiles", "sessionCount": 1 }
  ]
}
```

### Context Digest Builder

New module: `crates/core/src/report.rs`

```rust
pub struct ReportRequest {
    pub report_type: ReportType,     // Daily | Weekly | Custom
    pub date_start: NaiveDate,
    pub date_end: NaiveDate,
}

pub struct ContextDigest {
    pub summary_line: String,        // "8 sessions, 3 projects, 4h 12m, $6.80"
    pub projects: Vec<ProjectDigest>,
    pub top_tools: Vec<(String, u32)>,
    pub top_skills: Vec<(String, u32)>,
}

pub struct ProjectDigest {
    pub name: String,
    pub branches: Vec<BranchDigest>,
    pub commit_count: u32,
    pub files_edited: u32,
}

pub struct BranchDigest {
    pub name: String,
    pub sessions: Vec<SessionDigest>,
}

pub struct SessionDigest {
    pub first_prompt: String,        // truncated to ~100 chars
    pub classification: Option<String>,
    pub duration_mins: u32,
}
```

`ContextDigest::to_prompt_text(&self) -> String` formats into structured text:

```
=== Work Activity: Feb 21, 2026 ===

SUMMARY: 8 sessions, 3 projects, 4h 12m, $6.80

PROJECT: claude-view (5 sessions, 2h 48m)
  branch: feature/full-text-search
    - "Implement Tantivy indexer for session content" [feature, 45min]
    - "Fix search ranking to use BM25" [bugfix, 22min]
  branch: main
    - "Fix Mission Control SSE reconnect" [bugfix, 15min]
  commits: 3
  files edited: 12

PROJECT: vicky-wiki (2 sessions, 58m)
  branch: main
    - "Migrate auth pages to new layout" [refactor, 40min]
    - "Fix login redirect loop" [bugfix, 18min]
  commits: 1
  files edited: 6

TOOLS: Edit(47), Read(89), Bash(23), Grep(15)
SKILLS: brainstorming(1), TDD(2), debugging(1)
```

### Prompt Template

```
You are a concise work report writer.

Rules:
- 5-8 bullet points maximum
- Lead with VALUE (shipped, fixed, designed), not activity
- Group related work across sessions
- One summary line for metrics at the end
- No fluff, no filler words

Activity:
{digest.to_prompt_text()}

Write the report:
```

### CLI Streaming Pipeline

```
tokio::spawn --> spawn claude CLI with --print flag, prompt on stdin
                    |
                    v
              BufReader on stdout, read line-by-line
                    |
                    v
              tokio::sync::broadcast channel
                    |
                    v
              SSE handler reads from channel, emits "chunk" events
                    |
                    v
              on EOF: persist full content_md to DB, emit "done"
```

Env vars stripped per hard rule: `CLAUDECODE`, `CLAUDE_CODE_SSE_PORT`, `CLAUDE_CODE_ENTRYPOINT` + dynamic prefix scan.

### Crate Responsibilities

| Crate | New code |
|-------|----------|
| `core` | `report.rs` -- ReportType, ContextDigest, ProjectDigest, prompt template, to_prompt_text() |
| `db` | New migration, reports CRUD queries, get_report_preview() aggregate query |
| `server` | `routes/reports.rs` -- 5 endpoints, SSE streaming, CLI spawn + pipe |

---

## 4. Frontend Components

### Types

`src/types/generated/Report.ts` (auto-generated via ts-rs):

```ts
interface Report {
  id: number;
  reportType: 'daily' | 'weekly' | 'custom';
  dateStart: string;
  dateEnd: string;
  contentMd: string;
  sessionCount: number;
  projectCount: number;
  totalDurationSecs: number;
  totalCostCents: number;
  generationMs: number | null;
  createdAt: string;
}

interface ReportPreview {
  sessionCount: number;
  projectCount: number;
  totalDurationSecs: number;
  totalCostCents: number;
  projects: { name: string; sessionCount: number }[];
}
```

### Hooks

| Hook | Purpose | API Call |
|------|---------|----------|
| `useReportPreview(dateStart, dateEnd)` | Card preview stats | `GET /api/reports/preview` |
| `useReports()` | Report history list | `GET /api/reports` |
| `useReportGenerate()` | Trigger generation, manage SSE stream | `POST /api/reports/generate` |
| `useSmartDefaults()` | Compute time-aware primary/secondary cards | Pure client logic |

`useReportGenerate()` returns:
```ts
{
  generate: (req: GenerateRequest) => void,
  isGenerating: boolean,
  streamedText: string,   // accumulates as chunks arrive
  report: Report | null,  // set on "done" event
  error: string | null,
}
```

`useSmartDefaults()` returns:
```ts
{
  primary: { label: string, dateStart: string, dateEnd: string, type: ReportType },
  secondary: { label: string, dateStart: string, dateEnd: string, type: ReportType },
}
```

Logic:
- Morning (< noon) + today has < 2 sessions: primary = yesterday, secondary = today
- Monday morning: primary = last week, secondary = today
- Afternoon/evening: primary = today, secondary = this week

### Components

```
src/pages/ReportsPage.tsx              -- page entry, layout, smart defaults
src/components/reports/
  ReportCard.tsx                       -- the card (4 states: preview/streaming/complete/empty)
  ReportCardSkeleton.tsx               -- loading shimmer
  ReportContent.tsx                    -- markdown render + Copy/Export/Redo buttons
  ReportDetails.tsx                    -- expandable raw stats (no AI)
  ReportHistory.tsx                    -- history list below cards
```

### Routing

```ts
{ path: '/reports', element: <ReportsPage /> }
```

New sidebar nav item with Lucide `FileText` icon.

---

## 5. Error Handling & Edge Cases

### Error States

| Scenario | Handling |
|----------|----------|
| Claude CLI not installed | Card shows "Claude CLI required" with install link. No Generate button. |
| CLI not authenticated | "Run `claude auth login`" message |
| CLI timeout (>60s) | Kill child process. "Generation timed out. [Retry]". Don't persist partial. |
| CLI returns empty | "Empty response. [Retry]". Don't save to DB. |
| No sessions in date range | Empty state with nudge to nearest useful range |
| SSE connection dropped | Reconnect once. If fails, show "Connection lost. [Retry]" with partial text |
| Generation already running | Disable both Generate buttons. Server-side AtomicBool guard. |
| DB write fails after generation | Show report anyway. "Failed to save. [Copy] to preserve." |

### Edge Cases

| Case | Handling |
|------|----------|
| Midnight boundary | "Today" = calendar day in local TZ. Frontend sends TZ offset. |
| Week boundary | Monday 00:00 to Sunday 23:59 (ISO week) |
| Very long digest | Cap prompt context at ~4K tokens. Truncate least-active projects. |
| Rapid redo clicks | Cancel in-flight generation before starting new one |
| Multiple tabs | Server guard allows one generation at a time. Others see "in progress". |
| First-time user (0 sessions) | Full-page empty state: "No sessions found yet." |
| Future date range | Reject at API: dateEnd must be <= today |

### Security

- No user content in CLI prompt -- all data from our DB
- CLI stdout sanitized via DOMPurify before markdown render
- Sequential integer IDs, no auth (single-user local app)
