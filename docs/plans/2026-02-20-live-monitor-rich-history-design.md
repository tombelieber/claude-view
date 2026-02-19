# Live Monitor Rename + Rich History Sessions

**Date:** 2026-02-20
**Status:** Approved
**Branch:** feature/mission-control-cde

## Summary

Two changes:
1. Rename "Mission Control" to "Live Monitor" across all user-facing text and component names
2. Bring Mission Control's rich session data (cost breakdown, context gauge, cache stats, sub-agents) to the History detail page via pure JSONL parsing

## Architecture: Unified Accumulator (JSONL-only, no snapshots)

JSONL files are the single source of truth. All rich session data is reconstructed by parsing JSONL — no SQLite snapshots, no data duplication.

### Data flow

```
Live Monitor:   JSONL tail events → SessionAccumulator (streaming) → LiveSession → SSE
History:        JSONL full file   → SessionAccumulator (batch)     → RichSessionData → REST
```

### Core extraction

Extract `SessionAccumulator` from `crates/server/src/live/manager.rs` into `crates/core/src/accumulator.rs`:

```
crates/core/src/accumulator.rs  (NEW — extracted from manager.rs)
├── SessionAccumulator::new()
├── SessionAccumulator::process_line(LiveLine)         // streaming (live)
├── SessionAccumulator::from_file(path) → RichSessionData  // batch (history)
└── RichSessionData struct
    ├── tokens: TokenUsage
    ├── cost: CostBreakdown
    ├── cache_status: CacheStatus
    ├── sub_agents: Vec<SubAgentInfo>
    ├── progress_items: Vec<ProgressItem>
    ├── context_window_tokens: u64
    ├── model: Option<String>
    ├── git_branch: Option<String>
    ├── turn_count: u32
    ├── first_user_message: Option<String>
    └── last_user_message: Option<String>
```

### New REST endpoint

```
GET /api/sessions/:id/rich → RichSessionData
```

Parses the session's JSONL file through the shared accumulator. Performance: ~50-200ms for a 10MB file with mmap + SIMD pre-filter.

## Change 1: Rename "Mission Control" → "Live Monitor"

| Layer | Current | New |
|-------|---------|-----|
| Page title / nav label | "Mission Control" | "Live Monitor" |
| Page component | `MissionControlPage.tsx` | `LiveMonitorPage.tsx` |
| Internal types/comments | `MissionControl*` | `LiveMonitor*` |
| URL route | `/` (index) | `/` (unchanged) |
| API paths | `/api/live/*` | `/api/live/*` (unchanged) |
| Component dir | `src/components/live/` | `src/components/live/` (unchanged) |
| Design docs | `docs/plans/mission-control/` | Keep historical, no rename |

Backend API and component directory already use "live" — this is a frontend-only rename.

## Change 2: Rich History Detail Page

### Layout (two-column preserved)

```
┌──────────────────────────┬──────────────────────┐
│  [Continue ▾]  C/V  [Export ▾]                   │
├──────────────────────────┬──────────────────────┤
│                          │[Overview][Sub-Agents] │
│                          │[Cost]                 │
│  Conversation            ├──────────────────────┤
│  (compact/verbose)       │                      │
│                          │  (scrollable tab      │
│                          │   content)            │
│                          │                      │
└──────────────────────────┴──────────────────────┘
```

### Smart/Full → Compact/Verbose rename

| Current | New | Behavior |
|---------|-----|----------|
| Smart (Eye icon) | Compact (Eye icon) | Hides tool_use, tool_result, system, progress |
| Full (Code icon) | Verbose (Code icon) | Shows all messages |

Aligns with Live Monitor's RichPane toggle vocabulary.

### Right sidebar → Tabbed panel

Replace stacked sidebar sections with tabs:

**Overview tab** (default, scrollable):
```
┌─ Overview ──────────────┐
│ ■■■■■■■░░░ Context Peak │  ← NEW: context gauge
│ $2.34 total (saved $1.80)│  ← NEW: cost summary + cache savings
│ Model: claude-opus-4-6  │  ← NEW: model badge
│ Cache: 230k read tokens │  ← NEW: cache info
│─────────────────────────│
│ Prompts        12       │  ← existing SessionMetricsBar
│ Tokens     145,230      │
│ Files Read      8       │
│ Files Edited    5       │
│ Re-edits     12.5%      │
│ Commits         3       │
│─────────────────────────│
│ FILES TOUCHED           │  ← existing FilesTouchedPanel
│  auth.ts (3R 2E)        │
│  ...                    │
│─────────────────────────│
│ LINKED COMMITS          │  ← existing CommitsPanel
│  abc1234 fix auth bug   │
│  ...                    │
└─────────────────────────┘
```

**Sub-Agents tab** (conditional — hidden if no sub-agents):
- Reused from Live Monitor: sub-agent tree + swim lanes
- Shows: agent type, status, duration, tool count, cost
- Drill-down into individual agent conversations

**Cost tab** (always visible):
- Reused from Live Monitor: CostBreakdown component
- Detailed per-category costs (input, output, cache read, cache write)
- Cache tiering breakdown (5m vs 1hr TTL)
- Cache savings education ("Saved $X on Y cached tokens")

### Tab visibility

| Tab | When visible | Content |
|-----|-------------|---------|
| Overview | Always (default) | Merged rich data + existing stats + files + commits |
| Sub-Agents | Only if session had sub-agents | Tree + swim lanes |
| Cost | Always | Detailed breakdown + savings |

### Shared UI components (reused from Live Monitor)

| Component | Used in Live Monitor | Reused in History |
|-----------|---------------------|-------------------|
| `ContextGauge` | Live session card | Overview tab (peak value) |
| `CostBreakdown` | Cost tab | Cost tab |
| `CostTooltip` | Session card hover | Overview tab cost summary |
| `SwimLanes` | Sub-Agents tab | Sub-Agents tab |
| `SubAgentDrillDown` | Sub-Agents tab | Sub-Agents tab |

### Adaptations for history context

- Status badge: "Completed" (not "Working")
- Cache countdown: hidden (session ended)
- Context gauge: shows peak value, not live value
- No SSE streaming — single REST fetch
- Continue + Export buttons: completely untouched

## What is NOT changing

- Continue button and dropdown (copy transcript, resume command)
- Export button and dropdown (HTML, PDF, Markdown)
- Conversation message thread and virtualization
- Message filtering logic (just rename Smart→Compact, Full→Verbose)
- URL routes (`/sessions/:sessionId`)
- SQLite schema (no new columns)
- Existing REST endpoints for sessions

## UI/UX Guidelines Applied (UI/UX Pro Max)

- Dark mode: bg `#0F172A`/`#1E293B`, text `#F8FAFC`, CTA `#22C55E`
- Completed sessions: blue-500 badge (vs green for live)
- 4.5:1 contrast ratio on all text
- Visible focus rings on tabs
- Keyboard navigation for tab switching
- `prefers-reduced-motion` respected
- Touch targets min 44x44px
- No emojis as icons — use Lucide SVGs
- Smooth transitions 150-300ms
