# Message Types Full Alignment — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Achieve full alignment between the `docs/architecture/message-types.md` spec, the Rust parser, and ALL frontend UI surfaces (action log badges, verbose/debug chat, Rich/JSON mode, filter chips) — ensuring identical behavior in both chat history and live session views.

**Architecture:** Three layers must align: (1) Rust parser strips all 5 command tags and correctly classifies all 9 message types, (2) `message-to-rich.ts` converts all 7 JSONL roles to RichMessages with correct categories, (3) all UI surfaces — ActionRow badges, filter chips, RichPane renderers — handle all 13 ActionCategory variants with dedicated visual treatment. The shared component architecture (`RichPane`, `ActionFilterChips`, `SessionDetailPanel`, `ViewModeControls`) already serves both history and live contexts — the gaps are in lookup tables and state management, not structural.

**Tech Stack:** Rust (parser.rs), TypeScript/React (ActionRow.tsx, ActionLogTab.tsx, RichPane.tsx), Zustand (monitor-store.ts)

**Supersedes:** `docs/plans/2026-02-27-message-types-alignment.md` (original 2-task plan)

---

## Hook Terminology — Three Distinct Things (NEVER DEDUP)

| Name | Channel | What it is | Real data? | Status |
|------|---------|-----------|------------|--------|
| **`hook_progress`** | A (JSONL) | `"type":"progress"` with `data.type:"hook_progress"`. Real-time activity indicators written by Claude CLI to the session file. | Yes, common | Parsed → `Role::Progress`, category `hook_progress` |
| **`hook_events`** | B (HTTP POST → SQLite) | Lifecycle state transitions received via `POST /api/live/hook`. Stored in SQLite, pushed via WebSocket. | Yes, common | Stored in DB, served via REST + WS, category `hook` |
| ~~`hook_event`~~ (JSONL) | A (JSONL) | `"type":"hook_event"` as a top-level JSONL entry. | **Zero occurrences** across all real data (728,071 entries). Dead type. | **Removed from test fixtures. Not a real type.** |

**`hook_progress` and `hook_events` are DIFFERENT data from DIFFERENT channels. Both are shown in the timeline. NEVER deduplicate them.**

The JSONL `hook_event` type was a fabricated test fixture entry — Claude CLI has never produced one. It has been removed from `all_types.jsonl` and all test comments.

---

## Full Audit Summary

### Architecture Already Aligned (no changes needed)

These were verified by 3 parallel audit agents on 2026-02-27:

| Layer | Component | Status | Evidence |
|-------|-----------|--------|----------|
| Rust types | `Role` enum (7 variants), `ContentBlock` enum (5 variants) | MATCH | `types.rs:29-44, 583-603` |
| Rust categories | `categorize_tool()`, `categorize_progress()` | MATCH | `category.rs:10-39` |
| Rust parser | 9 named arms + wildcard | MATCH | `parser.rs:133-501` |
| TS types | `ActionCategory` (13 variants), `Role` (7 variants) | MATCH | `types.ts:1-14`, `Role.ts:6` |
| TS converter | `messagesToRichMessages()` — all 7 roles converted | MATCH | `message-to-rich.ts:46-170` |
| TS converter | `hookEventsToRichMessages()` — SQLite → RichMessage | MATCH | `hook-events-to-messages.ts:36-47` |
| TS converter | `hook_progress` category promotion | MATCH | `message-to-rich.ts:139-142` |
| RichPane | `RichMessage` interface (10 types, 8 fields) | MATCH | `RichPane.tsx:48-67` |
| RichPane | `MessageCard` switch — 9 cases + default | MATCH | `RichPane.tsx:812-833` |
| RichPane | `SystemMessageCard` — 9 subtypes + fallback | MATCH | `RichPane.tsx:511-630` |
| RichPane | `ProgressMessageCard` — 6 subtypes + hook_event + fallback | MATCH | `RichPane.tsx:646-746` |
| Filter chips | All 13 categories + "All" meta-chip | MATCH | `ActionFilterChips.tsx:4-27` |
| DB schema | `hook_events` — 8 columns, index on (session_id, timestamp) | MATCH | `migrations.rs:565-576` |
| Shared components | `RichPane`, `ViewModeControls`, `SessionDetailPanel` serve both history + live | MATCH | See architecture notes below |
| tool_use pairing | `usePairedMessages()` catches ALL tool_use → tool_pair before `MessageCard` | MATCH | `use-paired-messages.ts:14-54` |

### Items Confirmed NOT Issues

- **`tool_use` missing from `MessageCard` switch:** Not a bug. `usePairedMessages()` (line 929 of RichPane) wraps every `tool_use` into a `{ kind: 'tool_pair' }` `DisplayItem`. These render via `PairedToolCard`, never reaching `MessageCard`. The missing case is unreachable by design.
- **`hook` RichMessage never emitted by `messagesToRichMessages()`:** By design. In history, hook data arrives as `progress` messages (rendered by `HookProgressCard`) or SQLite hook events (rendered by `HookEventRow`). In live, WebSocket creates `hook` type (rendered by `HookMessage`). Different data sources → different render paths, both fully functional.
- **`result` parser arm:** Dead code (zero real data). Kept for forward compat per user decision.
- **Compact mode divergence:** History uses `MessageTyped` (full threading), live uses `RichPane` filtering. These are intentionally different — compact mode serves different UX goals in each context.

### Items Fixed Pre-Execution

- **JSONL `hook_event` removed from test fixture:** The `all_types.jsonl` fixture had a fabricated `{"type":"hook_event",...}` entry. Claude CLI has **never produced this type** (0 occurrences across 728,071 real entries). It was removed from the fixture and all test comments. The only real hook-related data are `hook_progress` (Channel A, JSONL progress subtype) and `hook_events` (Channel B, SQLite). These are **different data from different channels — NEVER deduplicate.**

### Gaps to Fix (5 tasks)

| # | Layer | Issue | Severity | Status |
|---|-------|-------|----------|--------|
| 0 | Test fixture | `all_types.jsonl` contained fabricated `hook_event` — not a real type | Medium | **DONE** |
| 1 | Rust | Parser strips 3/5 command tags | Medium | **DONE** |
| 2 | Frontend | `CATEGORY_BADGE` missing `context`, `result`, `summary` | Low | **DONE** |
| 3 | Frontend | `BADGE_LABELS` missing same 3 + `hook_progress` label = "Hook" (same as `hook`) | Low | **DONE** |
| 4 | Frontend | `ActionLogTab` filter chips use ephemeral local state; `RichPane` chips use global store — divergent | Low | **DONE** |

---

## Architecture Notes (for executor context)

### Shared Component Map

```
ViewModeControls (Chat/Debug + Rich/JSON toggle)
├── ConversationView header (history)
├── SessionDetailPanel tab bar (history + live)
└── TerminalOverlay header (live expanded)

RichPane (verbose/debug message renderer)
├── ConversationView → HistoryRichPane wrapper (history)
├── RichTerminalPane (live monitor grid)
└── SessionDetailPanel Terminal tab (history + live)

ActionFilterChips (13 category chips)
├── RichPane (when verboseMode=true) — uses global store: verboseFilter
└── ActionLogTab (Log tab) — uses LOCAL useState('all') ← THE DIVERGENCE

ActionLogTab (compact action timeline with ActionRow)
└── SessionDetailPanel Log tab (history + live)
```

### State Flow

```
monitor-store.ts (Zustand, persisted to localStorage)
├── verboseMode: boolean       → toggles Chat/Debug across ALL surfaces
├── verboseFilter: VerboseFilter → filters RichPane chips (global, persisted)
└── richRenderMode: 'rich'|'json' → toggles Rich/JSON across ALL surfaces

ActionLogTab (local)
└── activeFilter: useState('all') → filters Log tab chips (ephemeral, resets on mount)
```

Task 4 unifies the filter state so both surfaces use the same global store.

---

## Implementation Plan — 5 Tasks

### Task 0: Remove Fabricated `hook_event` from Test Fixture

**Status: DONE**

JSONL `"type":"hook_event"` is not a real type — Claude CLI has never produced one (0 occurrences across 728,071 real entries). The `all_types.jsonl` fixture had a fabricated entry for parser coverage. This was wrong — test fixtures must reflect real data, not invented types.

**Changes made:**
- `crates/core/tests/fixtures/all_types.jsonl` — removed line 14 (`hook_event` entry)
- `crates/core/src/parser.rs` — removed `hook_event` from test doc comments, updated count explanation

**Tests verified:** `test_parse_all_types_count` and `test_parse_meta_user_still_skipped` both pass (13 messages from 14 lines, 1 skipped isMeta).

**Hook terminology (canonical):**
- `hook_progress` = Channel A (JSONL `progress` with `data.type:"hook_progress"`). Real-time activity indicators. Parsed, displayed, category `hook_progress`.
- `hook_events` = Channel B (HTTP POST → SQLite/WebSocket). Lifecycle state transitions. Stored in DB, category `hook`.
- These are **DIFFERENT data from DIFFERENT channels. NEVER deduplicate.**

---

### Task 1: Commit Parser Tag-Stripping Fix (already implemented)

**Status: DONE** (committed as `063b0546`)

The code changes were made in the previous session and audit-fixed. All 3 call sites now pass 6 args to `clean_command_tags()`.

**Files already modified:**
- `crates/core/src/parser.rs` — 2 new regexes, updated function signature, 3 call sites, 2 new tests, `tag_regexes()` helper

**Step 1: Run tests**

```bash
cargo test -p claude-view-core parser::tests::test_clean_command_tags
```

Expected: 7 tests pass (5 existing + 2 new)

**Step 2: Full crate check**

```bash
cargo check -p claude-view-core
```

Expected: clean compilation, no warnings

**Step 3: Commit**

```bash
git add crates/core/src/parser.rs
git commit -m "fix(parser): strip all 5 command tags (was 3) — align with message-types.md §2.1

Add local_stdout_regex and system_reminder_regex to clean_command_tags().
All 3 call sites now pass all 5 tag regexes.
Fixes: <local-command-stdout> and <system-reminder> tags were leaking
into parsed user message content on the JSONL replay path."
```

---

### Task 2: Fix ActionRow Badge Maps (CATEGORY_BADGE + BADGE_LABELS)

**Status: DONE** (committed as `fed2522d`)

**Files:**
- Modify: `apps/web/src/components/live/action-log/ActionRow.tsx`

**Step 1: Add 3 missing `CATEGORY_BADGE` entries**

At `ActionRow.tsx:6-17`, add after the `queue` line (line 16):

```typescript
context: 'bg-emerald-500/10 text-emerald-400',
result: 'bg-green-500/10 text-green-400',
summary: 'bg-rose-500/10 text-rose-400',
```

The full map should now have 13 entries (matching all 13 `ActionCategory` variants). Colors sourced from `ActionFilterChips.tsx:20-26` (emerald/green/rose).

**Step 2: Fix `BADGE_LABELS` — disambiguate hook_progress, add 3 missing**

At `ActionRow.tsx:19-29`, replace line 25 and add after line 28:

Change line 25 from:
```typescript
hook_progress: 'Hook',
```
to:
```typescript
hook_progress: 'Hook Progress',
```

Add after the `queue` line (line 28):
```typescript
context: 'Context',
result: 'Result',
summary: 'Summary',
```

**Step 3: Verify build**

```bash
cd apps/web && bunx tsc --noEmit
```

Expected: no type errors

**Step 4: Commit**

```bash
git add apps/web/src/components/live/action-log/ActionRow.tsx
git commit -m "fix(ui): complete ActionRow badge maps for all 13 categories

Add CATEGORY_BADGE entries for context (emerald), result (green),
summary (rose). Add BADGE_LABELS for same 3. Disambiguate
hook_progress label from 'Hook' to 'Hook Progress'."
```

---

### Task 3: Unify ActionLogTab Filter State with Global Store

**Status: DONE** (committed as `14071235`)

**Why:** `RichPane` filter chips (Terminal tab) use `useMonitorStore.verboseFilter` (global, persisted). `ActionLogTab` filter chips (Log tab) use `useState('all')` (local, ephemeral, resets on tab switch). When you filter by "MCP" on the Terminal tab then switch to Log tab, the filter resets to "All". This is inconsistent — the same session data should have a unified filter across both tabs.

**Files:**
- Modify: `apps/web/src/components/live/action-log/ActionLogTab.tsx`

**Step 1: Replace local filter state with global store**

In `ActionLogTab.tsx`, replace the local state with the global store.

Change line 1 imports — add `useMonitorStore`:

```typescript
import { useMonitorStore } from '../../../store/monitor-store'
```

Remove the local state declaration at line 25:
```typescript
// DELETE: const [activeFilter, setActiveFilter] = useState<ActionCategory | 'all'>('all')
```

Add store selectors after line 24 (after `const allItems = useActionItems(messages)`):
```typescript
const activeFilter = useMonitorStore((s) => s.verboseFilter)
const setActiveFilter = useMonitorStore((s) => s.setVerboseFilter)
```

Remove `useState` from the react import at line 2 if it's no longer used elsewhere:
```typescript
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
```
Keep `useState` — it's still used for `atBottom` (line 27) and `showNewIndicator` (line 28).

**Step 2: Verify no type errors**

The `ActionFilterChips` component accepts `activeFilter: ActionCategory | 'all'` and `onFilterChange: (filter: ActionCategory | 'all') => void`. The store's `verboseFilter` is typed as `VerboseFilter = ActionCategory | 'all'` and `setVerboseFilter` accepts `(filter: VerboseFilter) => void`. Types align — no cast needed.

```bash
cd apps/web && bunx tsc --noEmit
```

Expected: no type errors

**Step 3: Commit**

```bash
git add apps/web/src/components/live/action-log/ActionLogTab.tsx
git commit -m "fix(ui): unify ActionLogTab filter state with global store

Replace local useState('all') with useMonitorStore.verboseFilter
so Terminal tab and Log tab share the same category filter.
Selecting 'MCP' on one tab now persists when switching to the other."
```

---

### Task 4: Build + End-to-End Verification

**Step 1: Full frontend build**

```bash
cd apps/web && bun run build
```

Expected: clean build, no errors

**Step 2: Run Rust tests (parser only — only crate modified)**

```bash
cargo test -p claude-view-core parser::tests::test_clean_command_tags
```

Expected: 7 passed

**Step 3: Visual verification checklist**

Start the dev server and verify in browser:

```bash
bun dev
```

**History session (chat history view):**
- [ ] Open a past session → toggle to Debug mode via ViewModeControls
- [ ] All 13 filter chips render with correct colors and counts
- [ ] Open the right-side panel (SessionDetailPanel) → click "Log" tab
- [ ] ActionRow badges show correct colors for all categories present
- [ ] `hook_progress` badge says "Hook Progress" (not "Hook")
- [ ] If `context`/`result`/`summary` entries exist, their badges show emerald/green/rose (not gray)
- [ ] Select a filter chip (e.g. "MCP") → switch between Terminal and Log tabs → filter persists
- [ ] Toggle Rich/JSON → both modes render all message types

**Live session (Mission Control):**
- [ ] Open a live session pane → toggle to Debug mode
- [ ] All 13 filter chips render
- [ ] Expand pane (TerminalOverlay) → Rich/JSON toggle visible
- [ ] Open SessionDetailPanel → Log tab → ActionRow badges correct
- [ ] Filter chip selection syncs between Terminal tab and Log tab

**Step 4: Commit verification results to plan**

Update this plan file's status fields to DONE.

---

## Verification Matrix — All 13 Categories × All Surfaces

After implementation, this matrix should be all green:

| Category | Filter Chip | Badge Color | Badge Label | RichPane Render | History | Live |
|----------|------------|-------------|-------------|----------------|---------|------|
| `builtin` | gray | gray | (tool name) | PairedToolCard | yes | yes |
| `mcp` | blue | blue | MCP | PairedToolCard | yes | yes |
| `skill` | purple | purple | Skill | PairedToolCard | yes | yes |
| `agent` | indigo | indigo | Agent | PairedToolCard | yes | yes |
| `hook` | amber | amber | Hook | HookMessage (live) / HookEventRow (history) | yes | yes |
| `hook_progress` | yellow | yellow | Hook Progress | HookProgressCard | yes | yes |
| `error` | red | red | Error | ErrorMessage | yes | yes |
| `system` | cyan | cyan | System | SystemMessageCard (9 subtypes) | yes | yes |
| `snapshot` | teal | teal | Snapshot | FileSnapshotCard | yes | yes |
| `queue` | orange | orange | Queue | MessageQueueEventCard | yes | yes |
| `context` | emerald | **emerald** (was gray) | **Context** (was raw) | SavedHookContextCard | yes | yes |
| `result` | green | **green** (was gray) | **Result** (was raw) | SessionResultCard | yes | yes |
| `summary` | rose | **rose** (was gray) | **Summary** (was raw) | SummaryMessageCard | yes | yes |

Bold = changed by this plan.

---

## Key Files Reference

| File | Purpose | Modified? |
|------|---------|-----------|
| `crates/core/tests/fixtures/all_types.jsonl` | Parser test fixture — all real JSONL types | Task 0 (done) |
| `crates/core/src/parser.rs` | JSONL parser — 9 match arms + 5 tag regexes | Task 0 + Task 1 |
| `apps/web/src/components/live/action-log/ActionRow.tsx` | CATEGORY_BADGE + BADGE_LABELS maps | Task 2 |
| `apps/web/src/components/live/action-log/ActionLogTab.tsx` | Log tab — filter state unification | Task 3 |
| `apps/web/src/store/monitor-store.ts` | Global state (verboseMode, verboseFilter, richRenderMode) | Not modified |
| `apps/web/src/components/live/RichPane.tsx` | Shared verbose renderer (history + live) | Not modified |
| `apps/web/src/components/live/ViewModeControls.tsx` | Shared Chat/Debug + Rich/JSON toggle | Not modified |
| `apps/web/src/components/live/action-log/ActionFilterChips.tsx` | 13 category chips + "All" | Not modified |
| `apps/web/src/components/live/SessionDetailPanel.tsx` | Shared detail panel (history + live) — Terminal + Log + Overview tabs | Not modified |
| `apps/web/src/components/ConversationView.tsx` | History page — HistoryRichPane + SessionDetailPanel inline | Not modified |
| `apps/web/src/lib/message-to-rich.ts` | JSONL → RichMessage converter (7 roles) | Not modified |
| `apps/web/src/lib/hook-events-to-messages.ts` | SQLite hook events → RichMessage | Not modified |
| `docs/architecture/message-types.md` | The spec (source of truth) | Not modified |

---

## Git State Warning

The working tree has pre-existing unstaged changes. Per CLAUDE.md git discipline: **only commit files YOU changed**.

Files to be committed by this plan:

- `crates/core/tests/fixtures/all_types.jsonl` — Task 0 (done)
- `crates/core/src/parser.rs` — Task 0 + Task 1
- `apps/web/src/components/live/action-log/ActionRow.tsx` — Task 2
- `apps/web/src/components/live/action-log/ActionLogTab.tsx` — Task 3
