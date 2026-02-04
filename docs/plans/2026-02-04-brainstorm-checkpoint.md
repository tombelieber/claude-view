---
status: done
date: 2026-02-04
purpose: Brainstorming checkpoint — all 4 themes designed
---

# Brainstorm Checkpoint: Themes 3 & 4

> Context: User feedback + personal observations from work machine (12GB data, ~6,700 sessions).
>
> **Completed:**
> - Theme 1 (Session Discovery & Navigation) — `2026-02-04-session-discovery-design.md`
> - Theme 2 (Dashboard & Analytics) — `2026-02-05-dashboard-analytics-design.md`
> - Theme 3 (Git Integration & AI Contribution) — `2026-02-05-theme3-git-ai-contribution-design.md`
> - Theme 4 (Chat Insights & Pattern Discovery) — `2026-02-05-theme4-chat-insights-design.md` + `theme4/` (8 phase plans)
>
> **All 4 themes designed.** Ready for implementation prioritization.

---

## Theme 2: Dashboard & Analytics Enhancements ✅ DESIGNED

> Status: **Design complete** — see `2026-02-05-dashboard-analytics-design.md`
> Design session: 2026-02-05

### Summary

| ID | Feature | Effort |
|----|---------|--------|
| 2A | Time range filter (header segmented control + API) | Medium |
| 2B | Heatmap hover tooltips | Small |
| 2C | Sync button redesign + toast notifications | Small |
| 2D | AI generation breakdown (new section, parser changes) | Large |
| 2E | Storage overview (settings page) | Medium |

**Recommended implementation order:** 2B → 2C → 2A → 2E → 2D

Full design with ASCII mockups, API specs, and acceptance criteria in dedicated plan file.

---

## Theme 3: Git Integration & AI Contribution Tracking ✅ DESIGNED

> Status: **Design complete** — see `2026-02-05-theme3-git-ai-contribution-design.md`
> Design session: 2026-02-05

### Summary

Dedicated `/contributions` page answering three questions:
1. **Fluency** — How skilled is the user with the tool?
2. **Volume** — How much AI output?
3. **Effectiveness** — How good is the AI output?

| Section | Description |
|---------|-------------|
| Overview Cards | Three pillars: Fluency / Output / Effectiveness |
| Contribution Trend | Line chart with human-readable insights |
| Efficiency Metrics | Cost vs output ROI, cost per line |
| Model Comparison | Which model gives best results |
| Learning Curve | Re-edit rate over time (improvement tracking) |
| By Branch | Sessions grouped by git branch with AI share % |
| Skill Effectiveness | Which skills lead to best outcomes |
| Uncommitted Work | Alert for AI code not yet committed |
| Session Drill-down | File-level breakdown, linked commits |

**Key design principles:**
- Every chart has human-readable insight line ("so what?")
- Work type badges: Deep Work, Quick Ask, Planning, Bug Fix (rule-based, no LLM)
- Data model: `ai_lines_added/removed` on sessions, `insertions/deletions` on commits

**Implementation order:** Data collection → API → Overview page → Dashboard card → Branch view → Advanced metrics

Full design with ASCII mockups, API specs, and acceptance criteria in dedicated plan file.

---

### Raw Feedback Items (for reference)

1. **AI coding contribution rate** — "User is very interested in finding the AI coding contribution rate — add/edit/remove lines of code, and files."

2. **Uncommitted work tracking** — "All git related works, be it committed or not, should have a way to detect or at least track. Let's discuss feasibility."

3. **Cursor-style dashboard** — "In Cursor, there's a very useful dashboard that tracks: AI share of committed code, Agent edits, Tab completions, Messages sent, % of cursor contribution in lines of code."

### Current State (for context when resuming)

- Git sync already exists (`POST /api/sync/git`) — scans repos, extracts commits, correlates to sessions by timestamp + skill usage.
- Commits stored in `commits` table, linked via `session_commits` junction table.
- Session already tracks: `commit_count`, `files_edited`, `files_read`, `tool_counts` (edit/read/bash/write).
- LOC estimation is planned in Theme 1 (Phase C + F) — tool-call estimate + git diff stats overlay.
- No concept of "AI share" vs "human share" of code currently.

### Key Feasibility Questions to Explore

- **AI share of code:** Claude Code sessions contain tool_use calls with exact Edit/Write content. We can attribute those lines to AI. But how to attribute human edits? Human edits happen outside Claude Code — we'd need to compare total git diff vs AI-attributed diff. Feasible but approximate.
- **Uncommitted work:** Tool_use calls in JSONL show what Claude wrote, whether or not it was committed. We already parse these. The gap is: did the user keep the changes or revert them? We can't know without git status at the time.
- **Tab completions:** Claude Code doesn't do tab completions (that's Cursor/Copilot). Not applicable.
- **Messages sent:** Already tracked as `user_prompt_count`.

### Realistic Scope (Claude Code vs Cursor)

| Cursor Metric | Claude Code Equivalent | Feasibility |
|--------------|----------------------|-------------|
| AI share of committed code | Compare AI tool_use LOC vs total commit LOC | Feasible — need git diff per commit + session LOC |
| Agent edits | `files_edited_count` + `tool_counts.edit` | Already tracked |
| Tab completions | N/A — Claude Code doesn't do this | Not applicable |
| Messages sent | `user_prompt_count` | Already tracked |
| % contribution | AI LOC / total LOC per time period | Feasible — derived metric |

---

## Theme 4: Chat Insights & Pattern Discovery ✅ DESIGNED

> Status: **Design complete** — see `2026-02-05-theme4-chat-insights-design.md` + `theme4/` directory
> Design session: 2026-02-05

### Summary

Two new pages (`/insights` and `/system`) with LLM-powered classification and 60+ pattern detection.

| Component | Description |
|-----------|-------------|
| Classification System | LLM-powered (Claude CLI default, BYOK), 30 categories in 3-level hierarchy |
| Pattern Engine | 60+ patterns across 8 categories (prompt, session, temporal, workflow, model, codebase, outcome, behavioral) |
| `/insights` Page | Hero insight, quick stats, 4 tabs (Patterns, Categories, Trends, Benchmarks) |
| `/system` Page | Storage/performance/health stats, index history, classification management |

**Key design decisions:**
- LLM classification via Claude CLI (`claude -p --output-format json`)
- Background async classification with SSE progress
- BYOK support (Anthropic API, OpenAI-compatible) for future monetization
- Every chart has human-readable "so what?" insight line

**8 Phase Plans (11,787 lines total):**

| Phase | Name | Lines | Parallelizable |
|-------|------|-------|----------------|
| 1 | Foundation | 1,060 | — |
| 2 | Classification System | 1,233 | with 3, 4 |
| 3 | System Page | 1,405 | with 2, 4 |
| 4 | Pattern Engine | 1,614 | with 2, 3 |
| 5 | Insights Core | 975 | — |
| 6 | Categories Tab | 1,782 | with 7, 8 |
| 7 | Trends Tab | 2,045 | with 6, 8 |
| 8 | Benchmarks Tab | 1,673 | with 6, 7 |

**Execution order:** Wave 1 (Phase 1) → Wave 2 (2+3+4 parallel) → Wave 3 (Phase 5) → Wave 4 (6+7+8 parallel)

Full designs with ASCII mockups, API specs, Rust types, React components, and acceptance criteria in `theme4/` directory.

---

## All Themes Complete

**Completed:**
- Theme 1: Session Discovery — `2026-02-04-session-discovery-design.md`
- Theme 2: Dashboard & Analytics — `2026-02-05-dashboard-analytics-design.md`
- Theme 3: Git Integration & AI Contribution — `2026-02-05-theme3-git-ai-contribution-design.md`
- Theme 4: Chat Insights & Pattern Discovery — `2026-02-05-theme4-chat-insights-design.md` + `theme4/`

**Next step:** Prioritize across all themes for implementation order.
