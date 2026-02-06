---
status: pending
date: 2026-02-05
purpose: Theme 3 Design — Git Integration & AI Contribution Tracking
---

# Theme 3: Git Integration & AI Contribution Tracking

> **Goal:** Help users understand how much AI is contributing to their codebase, track committed vs uncommitted work, and measure improvement over time.

## User Stories

| Persona | Question | Feature |
|---------|----------|---------|
| Individual Dev | "Am I getting better at using AI?" | Learning curve metrics |
| Individual Dev | "How much of my code is AI-written?" | AI contribution % |
| Tech Lead | "Where in the codebase is AI contributing most?" | Directory heatmap |
| Manager | "What's the ROI on AI tooling?" | Cost vs output metrics |

## Design Principles

### 1. Human-Readable Insights (Critical)

**Every chart, metric, or visualization MUST include a plain-English insight line.**

Users should NOT need to interpret data — the "so what?" should be stated plainly.

| Pattern | Example |
|---------|---------|
| **Comparison** | "Opus has 42% lower re-edit rate — better for complex work" |
| **Trend** | "You're getting 34% better at prompting over 6 months" |
| **Threshold** | "Your commit rate is above average (68% vs typical 55%)" |
| **Anomaly** | "Re-edit rate spiked this week — 3 sessions had >5 retries" |
| **Actionable** | "Tests have highest AI contribution — common pattern" |

### 2. Three Pillars Framework

All metrics organized under three dimensions:

| Pillar | Question | Key Metrics |
|--------|----------|-------------|
| **Fluency** | How skilled with the tool? | Sessions, prompts/session, tokens/line |
| **Volume** | How much AI output? | Lines +/-, files, commits |
| **Effectiveness** | How good is the output? | Commit rate, re-edit rate |

### 3. Work Type Classification

Rule-based classification (no LLM needed):

| Work Type | Heuristic | Badge |
|-----------|-----------|-------|
| **Deep Work** | duration > 30min, files_edited > 5, LOC > 200 | Blue |
| **Quick Ask** | duration < 5min, turn_count < 3, no edits | Lightning |
| **Planning** | skills contain "brainstorming" or "plan", low edits | Clipboard |
| **Bug Fix** | skills contain "debugging", moderate edits | Bug |
| **Standard** | Everything else | None |

---

## Information Architecture

```
/contributions
├── Header & Time Filter
├── Overview Cards (3 pillars)
├── Contribution Trend (chart)
├── Efficiency Metrics (ROI, Model comparison)
├── Learning Curve (progress over time)
├── By Branch (grouped view)
├── Skill Effectiveness (table)
├── Uncommitted Work (alerts)
└── Session Drill-down (expandable)
```

---

## Page Sections

### Section 1: Header & Time Filter

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  AI Contributions                                        [This Week ▼]      │
│                                                                             │
│  Tracking your AI-assisted development across 12 sessions                   │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Time range options:**
- Today
- This Week (default)
- This Month
- Last 90 Days
- All Time
- Custom Range

---

### Section 2: Overview Cards (Three Pillars)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐             │
│  │  FLUENCY        │  │  AI OUTPUT      │  │  EFFECTIVENESS  │             │
│  │                 │  │                 │  │                 │             │
│  │  12 sessions    │  │  +2,847 lines   │  │  68% committed  │             │
│  │  8.3 prompts/   │  │  -892 lines     │  │                 │             │
│  │  session avg    │  │  47 files       │  │  0.23 re-edit   │             │
│  │                 │  │                 │  │  rate           │             │
│  │  ↑ 15% vs last  │  │  → 6 commits    │  │                 │             │
│  │─────────────────│  │─────────────────│  │─────────────────│             │
│  │ More active     │  │ Your most       │  │ Below avg       │             │
│  │ than last week  │  │ productive week │  │ re-edit (0.30)  │             │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Metrics:**

| Card | Primary | Secondary | Insight Logic |
|------|---------|-----------|---------------|
| Fluency | Session count | Prompts/session, trend vs last period | Compare to previous period |
| AI Output | Lines +/- | Files touched, commits | Identify peak day |
| Effectiveness | Commit rate | Re-edit rate | Compare to benchmark (0.30) |

---

### Section 3: Contribution Trend Chart

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Contribution Trend                                                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1200 ┤                                           ╭──●                      │
│  1000 ┤                              ╭────────────╯                         │
│   800 ┤                    ╭─────────╯                                      │
│   600 ┤          ╭─────────╯                                                │
│   400 ┤    ╭─────╯                                                          │
│   200 ┤────╯                                                                │
│     0 ┼────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────    │
│       Mon  Tue  Wed  Thu  Fri  Sat  Sun  Mon  Tue  Wed  Thu  Fri  Sat      │
│                                                                             │
│  ── Added    ── Removed    ── Net                   [ Lines ▼ ] [ Commits ] │
│                                                                             │
│  AI output increased 2.3x this week — Thursday was peak (1,247 lines)      │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Toggle options:** Lines | Commits | Sessions | Files

---

### Section 4: Efficiency Metrics (ROI)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Efficiency                                                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  $12.47 spent  →  2,847 lines produced  →  6 commits shipped               │
│                                                                             │
│  Cost per line: $0.004    Cost per commit: $2.08                           │
│                                                                             │
│  Trend (last 4 weeks):                                                      │
│  $0.007 → $0.005 → $0.004 → $0.004                                         │
│                                                                             │
│  You're 43% more efficient than when you started — prompts are tighter     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Section 5: Model Comparison

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  By Model                                                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Model     Lines      Re-edit Rate    Cost/Line    Best For                │
│  ────────────────────────────────────────────────────────────────────────  │
│  Opus      1,423      0.18 ✓          $0.008       Complex features        │
│  Sonnet    892        0.31            $0.003       Standard work           │
│  Haiku     234        0.42            $0.001       Quick questions         │
│                                                                             │
│  Opus costs 2.7x more but needs 57% fewer re-edits — worth it for          │
│  complex work; use Sonnet for routine tasks to save cost                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Section 6: Learning Curve

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Your Progress                                                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Re-edit Rate Over Time (lower = better prompting)                         │
│                                                                             │
│  0.5 ┤████                                                                  │
│  0.4 ┤████████                                                              │
│  0.3 ┤████████████                        Your avg: 0.23                   │
│  0.2 ┤████████████████████ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─                         │
│  0.1 ┤████████████████████████████                                          │
│      └────────────────────────────────────────────────────────────────────  │
│       Jan    Feb    Mar    Apr    May    Jun                                │
│                                                                             │
│  Your re-edit rate dropped 54% since January — you're writing better       │
│  prompts that produce correct code on the first try                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Section 7: By Branch View

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  By Branch                                              [Sort: AI Lines ▼]  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─ feature/auth-flow ──────────────────────────────────────────────────┐  │
│  │  5 sessions  •  +1,247 / -312 lines  •  3 commits  •  Last: 2h ago   │  │
│  │  ████████████████████░░░░░░░░  78% AI share                          │  │
│  │                                                                       │  │
│  │  High AI share + high commit rate — AI doing heavy lifting here      │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌─ fix/pagination-bug ─────────────────────────────────────────────────┐  │
│  │  2 sessions  •  +89 / -34 lines  •  1 commit  •  Last: 1d ago        │  │
│  │  ██████████░░░░░░░░░░░░░░░░░░  42% AI share                          │  │
│  │                                                                       │  │
│  │  Lower AI share — likely more manual investigation/debugging         │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌─ main ───────────────────────────────────────────────────────────────┐  │
│  │  8 sessions  •  +423 / -178 lines  •  2 commits  •  Last: 3d ago     │  │
│  │  ████████████░░░░░░░░░░░░░░░░  52% AI share                          │  │
│  │                                                                       │  │
│  │  Balanced human/AI contribution on main branch                       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Clicking a branch** expands to show sessions or navigates to filtered session list.

---

### Section 8: Skill Effectiveness

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Skill Impact                                                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Skill          Sessions   Avg LOC   Commit Rate   Re-edit                 │
│  ─────────────────────────────────────────────────────────────────────────  │
│  tdd            12         +423      94%           0.12 ✓ best             │
│  brainstorming  8          +89       67%           0.28                     │
│  debugging      15         +67       88%           0.19                     │
│  commit         24         +156      100%          0.08 ✓ best             │
│  (no skill)     31         +234      61%           0.34   worst            │
│                                                                             │
│  Sessions using TDD skill have 65% lower re-edit rate than sessions        │
│  without skills — structured workflows produce better results              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Section 9: Uncommitted Work Tracker

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Uncommitted AI Work                                        [ Refresh ]     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─ claude-view (main) ─────────────────────────────────────────────────┐  │
│  │  +234 lines in 4 files  •  Last session: 2h ago                      │  │
│  │  Session: "Add contribution tracking API"                            │  │
│  │                                                                       │  │
│  │  Files: src/api/routes.rs, src/db/schema.rs, ...                     │  │
│  │                                                                       │  │
│  │  2 hours old — consider committing or this work may be lost          │  │
│  │                                              [ Dismiss ] [ View → ]  │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  You have 323 uncommitted AI lines across 2 projects — 12% of this         │
│  week's output. Commit often to avoid losing work.                         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Uncommitted calculation:**
- AI-generated LOC (from JSONL Edit/Write tool_use)
- Minus: Committed AI LOC (from correlated commits)
- Equals: Uncommitted (approximate — can't detect reverts/stashes)

---

### Section 10: Session Detail Expansion

When user clicks a session from branch view:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  ◀ feature/auth-flow                                                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Deep Work Session                                                          │
│  "Implement OAuth flow with refresh tokens"                                 │
│                                                                             │
│  ┌─────────────┬─────────────┬─────────────┬─────────────┐                 │
│  │  Duration   │  Prompts    │  AI Lines   │  Commits    │                 │
│  │  45 min     │  23         │  +847 / -123│  2          │                 │
│  └─────────────┴─────────────┴─────────────┴─────────────┘                 │
│                                                                             │
│  Files Impacted                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  src/auth/oauth.ts            +312 / -45   ███████████░░  created    │  │
│  │  src/auth/tokens.ts           +178 / -23   ████████░░░░░  created    │  │
│  │  src/api/middleware.ts        +89 / -12    ████░░░░░░░░░  modified   │  │
│  │  src/types/auth.d.ts          +67 / -0     ███░░░░░░░░░░  created    │  │
│  │  tests/auth.test.ts           +201 / -43   ████████░░░░░  created    │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  Linked Commits                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  abc1234  "feat: add OAuth provider integration"      +423 / -67     │  │
│  │  def5678  "feat: add token refresh middleware"        +312 / -45     │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  Effectiveness                                                              │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  ████████████████████░░░░░░░░  87% of AI code committed              │  │
│  │  Re-edit rate: 0.12 (low — good first-attempt accuracy)              │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│                                              [ Open Full Session → ]       │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

### Empty State

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                              [chart icon]                                   │
│                                                                             │
│                    No AI contributions this week                            │
│                                                                             │
│          Start a Claude Code session to see your                            │
│          AI-assisted development metrics here.                              │
│                                                                             │
│                         [ View All Time ]                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Model Changes

### Sessions Table (new columns)

```sql
ALTER TABLE sessions ADD COLUMN ai_lines_added INTEGER DEFAULT 0;
ALTER TABLE sessions ADD COLUMN ai_lines_removed INTEGER DEFAULT 0;
ALTER TABLE sessions ADD COLUMN work_type TEXT; -- 'deep_work', 'quick_ask', 'planning', 'bug_fix', 'standard'
```

### Commits Table (new columns)

```sql
ALTER TABLE commits ADD COLUMN files_changed INTEGER DEFAULT 0;
ALTER TABLE commits ADD COLUMN insertions INTEGER DEFAULT 0;
ALTER TABLE commits ADD COLUMN deletions INTEGER DEFAULT 0;
```

### New: contribution_snapshots Table (for trend data)

```sql
CREATE TABLE contribution_snapshots (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL,              -- YYYY-MM-DD
    project_id TEXT,                 -- NULL for global
    branch TEXT,                     -- NULL for project-wide
    sessions_count INTEGER DEFAULT 0,
    ai_lines_added INTEGER DEFAULT 0,
    ai_lines_removed INTEGER DEFAULT 0,
    commits_count INTEGER DEFAULT 0,
    commit_insertions INTEGER DEFAULT 0,
    commit_deletions INTEGER DEFAULT 0,
    tokens_used INTEGER DEFAULT 0,
    cost_cents INTEGER DEFAULT 0,
    UNIQUE(date, project_id, branch)
);
```

---

## API Endpoints

### GET /api/contributions

Main endpoint for the contributions page.

**Query params:**
- `range`: `today` | `week` | `month` | `90days` | `all` | `custom`
- `from`: ISO date (for custom range)
- `to`: ISO date (for custom range)
- `project_id`: optional filter

**Response:**
```typescript
interface ContributionsResponse {
  // Overview cards
  overview: {
    fluency: {
      sessions: number;
      promptsPerSession: number;
      trend: number; // % change vs previous period
      insight: string;
    };
    output: {
      linesAdded: number;
      linesRemoved: number;
      filesCount: number;
      commitsCount: number;
      insight: string;
    };
    effectiveness: {
      commitRate: number;
      reeditRate: number;
      insight: string;
    };
  };

  // Trend chart data
  trend: Array<{
    date: string;
    linesAdded: number;
    linesRemoved: number;
    commits: number;
    sessions: number;
  }>;

  // Efficiency metrics
  efficiency: {
    totalCost: number;
    totalLines: number;
    costPerLine: number;
    costPerCommit: number;
    costTrend: number[]; // last 4 periods
    insight: string;
  };

  // Model breakdown
  byModel: Array<{
    model: string;
    lines: number;
    reeditRate: number;
    costPerLine: number;
    insight: string;
  }>;

  // Learning curve
  learningCurve: {
    periods: Array<{ period: string; reeditRate: number }>;
    currentAvg: number;
    improvement: number; // % improvement since start
    insight: string;
  };

  // Branch grouping
  byBranch: Array<{
    branch: string;
    sessionsCount: number;
    linesAdded: number;
    linesRemoved: number;
    commitsCount: number;
    aiShare: number;
    lastActivity: string;
    insight: string;
  }>;

  // Skill effectiveness
  bySkill: Array<{
    skill: string;
    sessions: number;
    avgLoc: number;
    commitRate: number;
    reeditRate: number;
  }>;
  skillInsight: string;

  // Uncommitted work
  uncommitted: Array<{
    projectId: string;
    projectName: string;
    branch: string;
    linesAdded: number;
    filesCount: number;
    lastSessionId: string;
    lastSessionPreview: string;
    lastActivityAt: string;
    insight: string;
  }>;
  uncommittedInsight: string;
}
```

### GET /api/contributions/sessions/:id

Detailed contribution data for a single session.

**Response:**
```typescript
interface SessionContributionResponse {
  sessionId: string;
  workType: 'deep_work' | 'quick_ask' | 'planning' | 'bug_fix' | 'standard';
  duration: number;
  promptCount: number;

  // AI output
  aiLinesAdded: number;
  aiLinesRemoved: number;

  // Files breakdown
  files: Array<{
    path: string;
    linesAdded: number;
    linesRemoved: number;
    action: 'created' | 'modified' | 'deleted';
  }>;

  // Linked commits
  commits: Array<{
    hash: string;
    message: string;
    insertions: number;
    deletions: number;
    tier: 1 | 2;
  }>;

  // Effectiveness
  commitRate: number;
  reeditRate: number;
  insight: string;
}
```

---

## Metric Definitions

### Re-edit Rate

Measures how often AI output needs correction within a session.

```
re_edit_rate = edits_to_previously_edited_files / total_edits
```

**Calculation:**
1. Track all Edit/Write tool calls in a session
2. For each call, check if that file was already edited earlier in the session
3. Count repeat edits as "re-edits"

**Example:**
- Session has 10 Edit tool calls
- 3 target files already edited earlier in session
- Re-edit rate = 0.30

**Edge cases:**
- Zero edits in session → re-edit rate = `null` (display "—")
- Single edit → re-edit rate = 0.00

---

### AI Share (LOC-based)

Measures what percentage of committed code was AI-generated.

```
ai_share = ai_lines_added / total_commit_insertions
```

**Calculation:**
1. Sum `ai_lines_added` from all sessions linked to a branch
2. Sum `insertions` from all commits on that branch
3. Divide (capped at 1.0 — AI can't exceed 100%)

**Why LOC-based:**
- Directly answers "how much of my code is AI-written?"
- Matches user mental model of "78% AI share"
- File-based would be misleading (1 import across 10 files ≠ 100% AI)

**Edge cases:**
- Zero commits → ai_share = `null` (display "—")
- AI lines > commit lines (user deleted some) → cap at 1.0

---

### Cost Calculation

Estimated cost from token usage × model pricing.

```typescript
const PRICING_PER_MILLION = {
  'claude-opus-4': { input: 15, output: 75 },
  'claude-sonnet-4': { input: 3, output: 15 },
  'claude-haiku-3-5': { input: 0.25, output: 1.25 },
};

cost = (input_tokens * pricing.input + output_tokens * pricing.output) / 1_000_000;
```

**Data source:** JSONL entries contain `model` and `usage.input_tokens`, `usage.output_tokens`.

**UI disclaimer:** "Estimated cost based on token usage. Actual billing may vary."

**Pricing updates:** Hardcoded table, update with new releases. Consider config file for easier updates.

---

### Edge Case Handling

All percentage calculations use safe division:

```typescript
const safePct = (part: number, total: number): number | null => {
  if (total === 0) return null;
  return Math.min(1, part / total);
};
```

**Display rules:**
| Condition | Display |
|-----------|---------|
| Value is `null` | "—" |
| Learning curve < 2 periods | "Need more data" |
| No commits in period | Commit rate shows "—" |
| No sessions in period | Show empty state |

---

### Performance Strategy

**Pre-aggregation:** Daily snapshots computed nightly, not on-demand.

```
Raw JSONL + Git → Nightly Job → contribution_snapshots → API queries
```

**Query pattern:**
- "This week" → query 7 snapshot rows, not thousands of JSONL entries
- "Today" → compute real-time (only exception)

**Required indexes:**
```sql
CREATE INDEX idx_snapshots_date ON contribution_snapshots(date);
CREATE INDEX idx_snapshots_project_date ON contribution_snapshots(project_id, date);
CREATE INDEX idx_snapshots_branch_date ON contribution_snapshots(project_id, branch, date);
```

**Background job:** Runs after midnight local time, processes previous day's data.

---

### Error Handling

API returns partial data with warnings, never fails completely.

```typescript
interface ContributionsResponse {
  // ... existing fields
  warnings?: Array<{
    code: 'GIT_SYNC_INCOMPLETE' | 'COST_UNAVAILABLE' | 'PARTIAL_DATA';
    message: string;
  }>;
}
```

**Scenarios:**
| Error | Behavior |
|-------|----------|
| Git sync failed | Return session data, omit commit metrics, add warning |
| Missing token data | Omit cost metrics, add warning |
| Snapshot job didn't run | Compute on-demand (slower), add warning |

---

## Insight Generation Logic

```typescript
// Core insight generators
const insights = {
  fluency: (current: number, previous: number) => {
    const delta = ((current - previous) / previous) * 100;
    if (delta > 10) return `More active than last period (+${delta.toFixed(0)}%)`;
    if (delta < -10) return `Less active than last period (${delta.toFixed(0)}%)`;
    return `Consistent activity level`;
  },

  output: (lines: number, peak: { day: string; lines: number }) => {
    if (lines > 1000) return `Highly productive — ${peak.day} was peak (${peak.lines} lines)`;
    if (lines > 500) return `Good output — ${peak.day} was most active`;
    return `Light AI usage this period`;
  },

  effectiveness: (commitRate: number, reeditRate: number) => {
    if (commitRate > 0.8 && reeditRate < 0.2)
      return `Excellent — high commit rate, low re-edits`;
    if (reeditRate > 0.35)
      return `High re-edit rate — try more specific prompts`;
    if (commitRate < 0.5)
      return `Low commit rate — AI output may need more guidance`;
    return `Good balance of quality and throughput`;
  },

  model: (models: ModelStats[]) => {
    const best = models.reduce((a, b) => a.reeditRate < b.reeditRate ? a : b);
    const cheapest = models.reduce((a, b) => a.costPerLine < b.costPerLine ? a : b);
    if (best.model === cheapest.model)
      return `${best.model} is both cheapest and most accurate`;
    return `${best.model} has lowest re-edits; ${cheapest.model} is most cost-effective`;
  },

  learningCurve: (start: number, current: number) => {
    const improvement = ((start - current) / start) * 100;
    if (improvement > 30)
      return `Re-edit rate dropped ${improvement.toFixed(0)}% — your prompting has improved significantly`;
    if (improvement > 10)
      return `Steady improvement in prompt accuracy`;
    if (improvement < 0)
      return `Re-edit rate increasing — consider reviewing prompt patterns`;
    return `Consistent prompting quality`;
  },

  branch: (aiShare: number, commitRate: number) => {
    if (aiShare > 0.7 && commitRate > 0.8)
      return `High AI share + high commit rate — AI doing heavy lifting here`;
    if (aiShare < 0.3)
      return `Lower AI share — likely more manual investigation/debugging`;
    return `Balanced human/AI contribution`;
  },

  skill: (withSkill: number, withoutSkill: number) => {
    const improvement = ((withoutSkill - withSkill) / withoutSkill) * 100;
    if (improvement > 30)
      return `Sessions with skills have ${improvement.toFixed(0)}% lower re-edit rate — structured workflows help`;
    return `Skills provide modest improvement to output quality`;
  },

  uncommitted: (lines: number, totalLines: number, hours: number) => {
    const pct = (lines / totalLines) * 100;
    if (hours > 24)
      return `${lines} lines uncommitted for ${Math.floor(hours / 24)}+ days — consider committing`;
    if (pct > 20)
      return `${pct.toFixed(0)}% of recent work uncommitted — commit often to avoid losing work`;
    return `Small amount of uncommitted work`;
  }
};
```

---

## Integration Points

### Dashboard Enhancement

Add a summary card linking to `/contributions`:

```
┌─────────────────────────────────────────────────────────────────┐
│  AI Contribution This Week                      [ View All → ]  │
│                                                                 │
│  ████████████████████░░░░░░░░  72% AI-assisted                 │
│                                                                 │
│  +2,847 lines  •  6 commits  •  0.23 re-edit rate              │
│                                                                 │
│  Up 15% from last week — your most productive week             │
└─────────────────────────────────────────────────────────────────┘
```

### Session List Enhancement

- Add work type badges (Deep Work, Quick Ask, etc.)
- Add LOC column: `+234 / -12`
- Add filter: `Work Type: All | Deep Work | Quick Ask | Planning | Bug Fix`

### Session Detail Enhancement

- Add "Contribution" tab showing file-level AI impact
- Show linked commits with diff stats
- Show effectiveness metrics

---

## Implementation Order

| Phase | Scope | Effort |
|-------|-------|--------|
| **Phase 1** | Data collection: ai_lines in JSONL parser, diff stats in git sync | Medium |
| **Phase 2** | API: `/api/contributions` endpoint with basic metrics | Medium |
| **Phase 3** | UI: Contributions page with overview cards, trend chart | Large |
| **Phase 4** | UI: Branch grouping, session drill-down | Medium |
| **Phase 5** | UI: Efficiency metrics, model comparison, learning curve | Medium |
| **Phase 6** | Integration: Dashboard card, session list badges | Small |

**Recommended order:** Phase 1 → 2 → 3 → 6 → 4 → 5

(Get core metrics and dashboard integration working first, then add advanced views)

---

## Acceptance Criteria

### Must Have (MVP)
- [ ] AI lines +/- tracked per session during deep index
- [ ] Commit diff stats captured during git sync
- [ ] `/contributions` page with overview cards
- [ ] Trend chart with human-readable insight
- [ ] Time range filter (week/month/all)
- [ ] Every chart has insight line

### Should Have
- [ ] Branch grouping view
- [ ] Work type classification badges
- [ ] Session drill-down with file breakdown
- [ ] Uncommitted work alerts
- [ ] Dashboard summary card

### Nice to Have
- [ ] Model comparison table
- [ ] Learning curve chart
- [ ] Skill effectiveness table
- [ ] Cost/ROI metrics
- [ ] Directory heatmap (codebase coverage)

---

## Migration & Backfill Strategy

### Existing Data Backfill

When Theme 3 launches, existing sessions lack `ai_lines_added`, `ai_lines_removed`, and `work_type`. Backfill is required.

**Approach:** Re-parse JSONL files for existing sessions during next deep index.

```rust
// In deep_index.rs
async fn backfill_contribution_metrics(session: &mut Session, jsonl_path: &Path) -> Result<()> {
    if session.ai_lines_added.is_some() {
        return Ok(()); // Already computed, skip
    }

    let (lines_added, lines_removed) = count_ai_lines(jsonl_path).await?;
    let work_type = classify_work_type(session);

    session.ai_lines_added = Some(lines_added);
    session.ai_lines_removed = Some(lines_removed);
    session.work_type = Some(work_type);

    Ok(())
}
```

**Backfill triggers:**

1. **Automatic:** Next scheduled deep index after upgrade
2. **Manual:** `vibe-recall reindex --backfill-contributions`

**Progress tracking:**

- Store `last_backfill_version` in DB metadata
- Current version: `1` (initial release)
- Future schema changes increment version, triggering re-backfill

### Snapshot Bootstrap

First run after upgrade has no historical snapshots. Bootstrap strategy:

```sql
-- Generate snapshots for all historical dates with sessions
INSERT INTO contribution_snapshots (date, project_id, branch, ...)
SELECT
    DATE(started_at) as date,
    project_id,
    branch,
    COUNT(*) as sessions_count,
    SUM(ai_lines_added) as ai_lines_added,
    ...
FROM sessions
WHERE ai_lines_added IS NOT NULL
GROUP BY DATE(started_at), project_id, branch;
```

**Bootstrap runs once** during first snapshot job execution after upgrade.

---

## Test Strategy

### Unit Tests

| Component | Test Focus | Location |
|-----------|------------|----------|
| `count_ai_lines()` | Parses Edit/Write tool_use, handles malformed JSONL | `crates/core/src/contribution_test.rs` |
| `classify_work_type()` | Heuristic boundaries, edge cases | `crates/core/src/work_type_test.rs` |
| `insights::fluency()` | Trend calculation, zero-division | `crates/server/src/insights_test.rs` |
| `insights::effectiveness()` | Threshold logic, null handling | `crates/server/src/insights_test.rs` |
| `safe_pct()` | Division by zero returns null | `crates/core/src/math_test.rs` |

**Test fixtures:**

- `fixtures/sessions/deep_work.jsonl` — 45min, 847 lines, 23 prompts
- `fixtures/sessions/quick_ask.jsonl` — 2min, 0 edits, 2 prompts
- `fixtures/sessions/planning.jsonl` — brainstorming skill, low edits
- `fixtures/sessions/empty.jsonl` — no tool calls

### Integration Tests

| Scenario | Test File |
|----------|-----------|
| `/api/contributions` returns correct aggregates | `crates/server/tests/contributions_api.rs` |
| Snapshot job produces correct daily rollups | `crates/db/tests/snapshot_job.rs` |
| Git sync captures diff stats | `crates/core/tests/git_sync.rs` |
| Backfill populates missing fields | `crates/db/tests/backfill.rs` |

**Test database:** Use SQLite in-memory with seeded test data.

### E2E Tests

| Flow | Tool |
|------|------|
| Load `/contributions`, verify cards render | Playwright |
| Change time filter, verify chart updates | Playwright |
| Click branch, verify drill-down opens | Playwright |
| Empty state displays when no sessions | Playwright |

**E2E test location:** `frontend/e2e/contributions.spec.ts`

### Test Commands

```bash
# Unit tests for contribution metrics
cargo test -p core -- contribution

# Integration tests for API
cargo test -p server -- contributions

# E2E tests
pnpm --filter frontend test:e2e -- contributions
```

---

## Performance Benchmarks

### API Latency Targets

| Endpoint | P50 | P95 | P99 |
|----------|-----|-----|-----|
| `GET /api/contributions?range=week` | < 50ms | < 100ms | < 200ms |
| `GET /api/contributions?range=month` | < 100ms | < 200ms | < 400ms |
| `GET /api/contributions?range=all` | < 200ms | < 500ms | < 1s |
| `GET /api/contributions/sessions/:id` | < 30ms | < 50ms | < 100ms |

**Measurement:** Add tracing spans, emit metrics to logs.

### Memory Limits

| Operation | Max Memory |
|-----------|------------|
| Snapshot aggregation query | < 50MB |
| Single session contribution parse | < 10MB |
| Backfill batch (100 sessions) | < 200MB |

### Snapshot Job Performance

| Metric | Target |
|--------|--------|
| Daily snapshot generation | < 5s for 1000 sessions |
| Bootstrap (first run, 10k sessions) | < 60s |

### Frontend Bundle Impact

| Metric | Budget |
|--------|--------|
| Chart library (Recharts) | < 50KB gzipped |
| Contributions page chunk | < 30KB gzipped |
| Total JS increase | < 80KB gzipped |

**Chart library choice:** Recharts (already commonly used, tree-shakeable).

---

## Frontend Component Tree

```
frontend/src/pages/ContributionsPage.tsx
├── ContributionsHeader.tsx
│   ├── PageTitle
│   └── TimeRangeFilter.tsx (dropdown)
│
├── OverviewCards.tsx
│   ├── FluencyCard.tsx
│   ├── OutputCard.tsx
│   └── EffectivenessCard.tsx
│
├── TrendChart.tsx
│   ├── Chart (Recharts LineChart)
│   ├── ChartToggle.tsx (Lines/Commits/Sessions)
│   └── InsightLine.tsx
│
├── EfficiencyMetrics.tsx
│   └── CostBreakdown.tsx
│
├── ModelComparison.tsx
│   └── ModelTable.tsx
│
├── LearningCurve.tsx
│   └── ProgressChart.tsx (Recharts BarChart)
│
├── BranchList.tsx
│   └── BranchCard.tsx (expandable)
│       └── SessionSummary.tsx
│
├── SkillEffectiveness.tsx
│   └── SkillTable.tsx
│
├── UncommittedWork.tsx
│   └── UncommittedCard.tsx
│
├── SessionDrillDown.tsx (modal or slide-over)
│   ├── SessionHeader.tsx
│   ├── FileImpactList.tsx
│   ├── LinkedCommits.tsx
│   └── EffectivenessBar.tsx
│
└── ContributionsEmptyState.tsx
```

### Shared Components

| Component | Purpose |
|-----------|---------|
| `InsightLine.tsx` | Renders plain-English insight with icon |
| `MetricCard.tsx` | Reusable card with primary/secondary values |
| `ProgressBar.tsx` | Horizontal bar for percentages |
| `TrendIndicator.tsx` | Up/down arrow with % change |
| `WorkTypeBadge.tsx` | Colored badge for work type |

### State Management

```typescript
// frontend/src/hooks/useContributions.ts
export function useContributions(range: TimeRange) {
  return useQuery({
    queryKey: ['contributions', range],
    queryFn: () => api.getContributions(range),
    staleTime: 5 * 60 * 1000, // 5 min
    gcTime: 30 * 60 * 1000,   // 30 min
  });
}
```

**Library:** TanStack Query (React Query) for caching and background refresh.

---

## Caching Strategy

### API Response Caching

| Endpoint | Cache Duration | Invalidation |
|----------|----------------|--------------|
| `/api/contributions?range=today` | 1 min | On new session |
| `/api/contributions?range=week` | 5 min | On snapshot job |
| `/api/contributions?range=month` | 15 min | On snapshot job |
| `/api/contributions?range=all` | 30 min | On snapshot job |

**Implementation:** HTTP `Cache-Control` headers + ETag.

```rust
// In contributions route
let cache_seconds = match range {
    TimeRange::Today => 60,
    TimeRange::Week => 300,
    TimeRange::Month => 900,
    _ => 1800,
};

Response::builder()
    .header("Cache-Control", format!("max-age={}", cache_seconds))
    .header("ETag", compute_etag(&data))
    .body(Json(data))
```

### Frontend Caching

- **React Query** handles client-side cache
- `staleTime`: Match API cache duration
- `gcTime`: 30 minutes (keep in memory for back-navigation)
- **Optimistic updates:** Not needed (read-only page)

### Snapshot Job Cache Invalidation

After daily snapshot job completes:
1. Bump `snapshot_version` in DB metadata
2. Next API request sees version mismatch → returns fresh data
3. ETag changes → client refetches

---

## Snapshot Retention Policy

| Granularity | Retention |
|-------------|-----------|
| Daily snapshots | 90 days |
| Weekly rollups | 1 year |
| Monthly rollups | Forever |

**Rollup job:** Runs weekly, aggregates old daily snapshots into weekly/monthly.

```sql
-- Weekly rollup (runs every Sunday)
INSERT INTO contribution_snapshots (date, project_id, branch, granularity, ...)
SELECT
    DATE(date, 'weekday 0', '-7 days') as week_start,
    project_id,
    branch,
    'weekly' as granularity,
    SUM(sessions_count),
    ...
FROM contribution_snapshots
WHERE granularity = 'daily'
  AND date < DATE('now', '-90 days')
GROUP BY week_start, project_id, branch;

-- Delete old daily snapshots
DELETE FROM contribution_snapshots
WHERE granularity = 'daily'
  AND date < DATE('now', '-90 days');
```

### Session Data Retention

Session contribution fields (`ai_lines_added`, `work_type`) follow existing session retention policy (no separate policy needed).

---

## Error States UX

### API Errors

| Error | UI Behavior |
|-------|-------------|
| 500 Internal Error | Show error banner with "Retry" button, preserve last-good data if cached |
| 503 Indexing In Progress | Show "Building contribution data..." with spinner |
| Partial data (warnings) | Show data + warning banner explaining what's missing |

### Specific Warning States

| Warning Code | User Message |
|--------------|--------------|
| `GIT_SYNC_INCOMPLETE` | "Some commit data unavailable — run `vibe-recall sync` to update" |
| `COST_UNAVAILABLE` | "Cost metrics unavailable — token data missing from some sessions" |
| `PARTIAL_DATA` | "Showing partial data — some sessions still indexing" |

### Empty States

| Condition | UI |
|-----------|-----|
| No sessions in time range | Empty state with "View All Time" button |
| No commits linked | Show session data, hide commit-dependent metrics, show "No commits found" |
| New user (0 sessions) | Onboarding empty state with "Start a Claude Code session..." |
