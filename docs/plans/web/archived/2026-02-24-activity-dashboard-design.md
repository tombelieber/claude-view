# Activity Dashboard Design

**Date:** 2026-02-24
**Status:** Approved
**Route:** `/activity`

## Purpose

A "where did my time go?" dashboard for Claude Code users. Combines three views:
1. **Summary stats** — total time, session count, avg session, week-over-week comparison
2. **Calendar heatmap** — GitHub-style intensity grid showing daily usage
3. **Project breakdown** — horizontal bars showing time-by-project
4. **Daily timeline** — session journal showing each session's start/end/project/duration

## Core Design Principle

Ship all three intents (awareness, productivity, journal) and let the market decide what to keep, polish, or cut.

## Layout (top to bottom)

### Section 1: Summary Stats Bar

KPI cards at the top, always visible:
- Total time this period
- Session count
- Average session length
- Busiest day
- Week-over-week change (delta + percentage)
- Longest session (project + duration)

Time range picker: Today / This Week / This Month / Custom

### Section 2: Calendar Heatmap

GitHub contribution-style grid:
- Rows = days of week (Mon–Fri or Mon–Sun)
- Columns = weeks
- Cell intensity = total hours that day
- Hover → tooltip: "Tuesday Feb 18: 4h 22m across 5 sessions"
- Click → scrolls to daily timeline, filtered to that day
- Legend: ░ <1h, ▒ 1-3h, ▓ 3h+
- Month navigation: ◀ / ▶

Implementation: Pure CSS grid + Tailwind (no extra dependency). Each cell is a colored div with a tooltip.

### Section 3: By-Project Breakdown

Horizontal bar chart:
- One row per project, sorted by total time descending
- Bar width proportional to time
- Label: project name + duration + percentage
- Click a project → filters the daily timeline below

Implementation: Recharts `<BarChart layout="vertical">` (already installed).

### Section 4: Daily Timeline

Session journal, grouped by day:
- Day header: "Today — Mon Feb 24 (3 sessions, 4h 30m)"
- Each row: start→end time, project name, session title, duration
- Click a session → navigates to `/sessions/:id`
- Lazy-loads older days on scroll ("Load more days..." button)
- Filtered by heatmap day click or project bar click

Implementation: Virtualized list for performance. Reuse existing session data hooks.

## Navigation

Add "Activity" to sidebar between "Analytics" and "Reports":
```
Live Monitor
Sessions
Analytics
Activity    ← NEW
Reports
```

Icon: `Activity` from lucide-react (or `Clock` / `CalendarDays`)

## Data Requirements

### Existing data (no backend changes for V1)

All data available from `GET /api/sessions`:
- `duration_seconds` — session duration
- `first_message_at` / `last_message_at` — timestamps
- `project_path` — project grouping. After CWD resolution fix (`2026-02-25-cwd-resolution-fix.md`): always sourced from `cwd` in JSONL — accurate and authoritative. No DFS guessing, no garbled `@`/`.`/hyphen names.
- `title` — session title for timeline display

API already supports `time_after`, `time_before` filtering and `duration` sorting.

### Session scope (depends on reliability release)

After the reliability release fixes session classification:

- `/api/sessions` returns only `kind=Conversation` sessions. Metadata files (file-history-snapshot, summary, queue-operation) and subagent transcripts are excluded at the API layer — no client-side filtering needed.
- **Fork/continuation sessions (`parent_id IS NOT NULL`) are intentionally counted.** Each fork is an independent working session with its own `duration_seconds`. Counting them gives accurate total working time. Excluding them would silently under-count.
- Activity stats therefore represent total conversation time across all sessions — root sessions and their forks alike.

The activity dashboard functions correctly before the reliability release but will produce more accurate project names and session counts after it ships.

### V1 approach: Client-side aggregation

Fetch sessions for the selected time range via existing `/api/sessions?time_after=X&time_before=Y` and compute aggregations in the browser:
- Group by day for heatmap
- Group by project for breakdown bars
- Sort by time for timeline

### Future optimization (V2)

Add `GET /api/activity?range=week&offset=0` server-side aggregation endpoint if client-side becomes slow with large datasets.

## Tech Stack

- **Charts:** Recharts (already installed) for bar charts
- **Heatmap:** Pure CSS grid + Tailwind (no extra dependency)
- **Icons:** Lucide React (already installed)
- **State:** React hooks + existing session data hooks
- **Routing:** React Router (add `/activity` route)

## Dark Mode

Follow existing claude-view patterns:
- Use explicit Tailwind + `dark:` variants (no shadcn/ui CSS vars)
- Heatmap cells: gray-100/200/300 light → gray-800/700/600 dark
- Bar chart: blue-500 light → blue-400 dark
- Background: gray-50 light → gray-950 dark

## Not in V1 (YAGNI)

- Goal setting / time budgets
- Export to CSV
- Pomodoro integration
- Notifications / alerts
- Team member comparison
- Streaks / gamification
- Per-turn timeline within sessions (already exists in session detail view)
