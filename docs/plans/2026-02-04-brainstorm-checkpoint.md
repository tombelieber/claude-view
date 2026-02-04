---
status: draft
date: 2026-02-04
purpose: Brainstorming checkpoint — resume point for Theme 3 & 4 design sessions
---

# Brainstorm Checkpoint: Themes 3 & 4

> Context: User feedback + personal observations from work machine (12GB data, ~6,700 sessions).
>
> **Completed:**
> - Theme 1 (Session Discovery & Navigation) — `2026-02-04-session-discovery-design.md`
> - Theme 2 (Dashboard & Analytics) — `2026-02-05-dashboard-analytics-design.md`
>
> **Remaining:** Themes 3 & 4 need their own design documents.

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

## Theme 3: Git Integration & AI Contribution Tracking

### Raw Feedback Items

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

## Theme 4: Chat Insights & Pattern Discovery

### Raw Feedback Items

1. **Session classification** — "Work type: bug, feature, KTLO. Intent distribution: ask, plan, task automation, write code. Categories: doc, feat, architecture, refactor, bug fix, explanation, testing, config, ops & deployment. Task complexity: trivial/low/medium/high. Prompt specificity: high/low/medium."

2. **Chat pattern discovery** — "The main purpose is to find out how I can improve next time. Is there some pattern from my prompts you can tell? By just looking at and analyzing my prompts only (without LLM response), we can have a pretty good intent analysis and pattern insights."

3. **Nerd stats / performance page** — "It only took 3s to do deep index on my work machine, well done. Should we have a page for these nerd numbers? User can check, test, manage — clean cache (the index), re-index, show elapsed time."

4. **Time to market is important** — User has free open source models + free GPU for inference. Cost of insight matters but local inference is an option.

### Current State (for context when resuming)

- Session `preview` and `lastMessage` already captured — these are the user's first and last prompts.
- `skills_used` tracked — partially indicates intent (e.g., `tdd`, `brainstorming`, `commit`).
- `summary_text` field exists on sessions (from full parser) — could contain AI-generated summary.
- No classification/categorization system exists.
- No prompt pattern analysis exists.
- Deep index timing is logged in debug builds but not exposed to users.
- Index metadata available via `GET /api/status` — `lastIndexedAt`, `sessionsIndexed`, `lastGitSyncAt`, `commitsFound`.

### Design Questions to Explore

- **Classification approach:** Rule-based (keyword matching on prompts) vs LLM-based (local inference) vs hybrid? Rule-based is fast and free. LLM-based is more accurate but adds dependency.
- **Where to classify:** During deep index (backend, Rust) or on-demand (frontend/separate service)?
- **Pattern discovery:** What patterns are useful? Prompt length trends, time-of-day patterns, session length distribution, skill usage frequency over time, common file types?
- **Nerd stats page:** Settings subpage or standalone page? What metrics: index time, DB size, session count, JSONL total size, parse throughput (MB/s)?

### User's Key Insight

> "By just looking at and analyzing my prompts only without the response from LLM, actually we can have a pretty good intent analysis."

This suggests a lightweight approach: classify based on the user's prompt text alone, not the full conversation. This is much cheaper computationally and avoids needing to parse assistant responses for classification.

---

## Process for Resuming

**Completed:**
- Theme 1: Session Discovery — `2026-02-04-session-discovery-design.md`
- Theme 2: Dashboard & Analytics — `2026-02-05-dashboard-analytics-design.md`

**Remaining:**
1. Pick Theme 3 or 4
2. Use `superpowers:brainstorming` skill — same process as Themes 1 & 2
3. Ask clarifying questions one at a time
4. Present design in sections, validate each
5. Write to dedicated plan file
6. After all 4 themes designed, prioritize across all plans for implementation order
