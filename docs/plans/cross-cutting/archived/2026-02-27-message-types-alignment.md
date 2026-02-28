# Message-Types.md Alignment — Audit Report & Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Align codebase (Rust + frontend) with the updated `docs/architecture/message-types.md` spec and fix UI gaps in filter chips/verbose mode.

**Architecture:** The doc is the source of truth. Discrepancies fall into two buckets: (A) code that doesn't match doc claims, (B) UI gaps where categories exist in the type system but have no visual treatment.

**Tech Stack:** Rust (parser.rs), TypeScript/React (ActionRow.tsx)

**Date:** 2026-02-27

---

## Session Summary

A comprehensive audit was performed comparing `docs/architecture/message-types.md` (the spec) against the actual codebase. Three parallel exploration agents audited: (1) Rust backend, (2) Frontend types/components, (3) UI filter chips/verbose mode.

---

## Full Audit Results

### What MATCHES the doc (no changes needed)

| Item | Doc Claim | Code | Status |
|------|-----------|------|--------|
| `Role` enum (Rust) | 7 variants: User, Assistant, ToolUse, ToolResult, System, Progress, Summary | `crates/core/src/types.rs:29-44` | EXACT MATCH |
| `ContentBlock` enum | 5 variants: Text, Thinking, ToolUse, ToolResult, Other | `crates/core/src/types.rs:583-603` | EXACT MATCH |
| `categorize_tool()` | Skill→skill, mcp__/mcp_→mcp, Task→agent, fallback→builtin | `crates/core/src/category.rs:10-39` | EXACT MATCH |
| `categorize_progress()` | hook_progress→hook, agent_progress/waiting_for_task→agent, bash_progress→builtin, mcp_progress→mcp | `crates/core/src/category.rs` | EXACT MATCH |
| `RichMessage` interface | 10 type variants, 8 fields | `apps/web/src/components/live/RichPane.tsx:48-67` | EXACT MATCH |
| `ActionCategory` type | 13 variants | `apps/web/src/components/live/action-log/types.ts:1-14` | EXACT MATCH |
| `Role` TS type | 7-variant union | `apps/web/src/types/generated/Role.ts:6` | EXACT MATCH |
| `hookEventsToRichMessages()` | type:'progress', category:'hook', metadata.type:'hook_event', metadata._hookEvent | `apps/web/src/lib/hook-events-to-messages.ts:36-47` | EXACT MATCH |
| `message-to-rich.ts` hook_progress override | Overrides Rust "hook" to "hook_progress" for filter chips | `apps/web/src/lib/message-to-rich.ts:139-142` | EXACT MATCH |
| hook_events DB schema | 8 columns (incl. `id` PK), index on (session_id, timestamp) | `crates/db/src/migrations.rs` | EXACT MATCH |
| In-memory hook limit | max 5000, oldest 100 dropped | `crates/server/src/routes/hooks.rs:47,518-519` | EXACT MATCH |
| All 13 filter chips present | All ActionCategory variants have chips | `apps/web/src/components/live/action-log/ActionFilterChips.tsx:4-27` | EXACT MATCH |
| `context` category wiring | Rust assigns "context" to saved_hook_context, frontend preserves via `msg.category ?? 'system'` | `message-to-rich.ts:130` | WORKS (shows 0 when no entries, expected) |
| Parser entry count | "9 known types" + hook_event/pr-link ignored via wildcard | `crates/core/src/parser.rs:129-499` — 9 named arms + wildcard | EXACT MATCH |
| `hook_event` handling | "(ignored)" — falls to wildcard | `parser.rs` wildcard at line 493 | EXACT MATCH |

### Discrepancies Found (CODE changes needed)

| # | Layer | Issue | Severity | Details |
|---|-------|-------|----------|---------|
| 1 | Rust | `parser.rs` strips 3 command tags; doc+frontend strip 5 | **Medium** | Doc §2.1 line 82 says 5 tags stripped: `<command-name>`, `<command-args>`, `<command-message>`, `<local-command-stdout>`, `<system-reminder>`. Rust parser (`parser.rs:74-77`) only has 3 regexes. Frontend `message-to-rich.ts:7-14` already strips all 5 (redundant safety net for WebSocket path). |
| 2 | Frontend | `CATEGORY_BADGE` missing 3 entries | **Low** | `ActionRow.tsx:6-17` has 10 entries. Missing: `context`, `result`, `summary`. These 3 categories fall to `CATEGORY_BADGE.builtin` (gray) instead of matching their chip colors (emerald, green, rose). |
| 3 | Frontend | `BADGE_LABELS` — `hook_progress` indistinguishable from `hook` | **Low** | `ActionRow.tsx:25` maps `hook_progress` to "Hook" — same label as `hook` on line 24. Colors (amber vs yellow) are nearly indistinguishable. |
| 4 | Frontend | `BADGE_LABELS` missing 3 entries | **Low** | Missing: `context`, `result`, `summary`. These show raw category strings as badge text instead of proper labels. |

### Items Confirmed NOT Issues

- **`result` parser arm:** Dead code (zero real data, 3 lines at `parser.rs:470-492`). Kept for forward compat per user decision.
- **`hook_event` parser arm:** Doc was updated to say "(ignored)" — matches code. Originally the doc described a forward-compat parser arm, but user removed that from the doc.
- **`context` chip:** Not dead — works when `saved_hook_context` entries exist in a session.

### Notable Undocumented Code Behaviors (doc omissions, NOT bugs)

These were found during audit but the user chose not to add them to the doc:

| Behavior | Location | Description |
|----------|----------|-------------|
| `source=="clear"` for SessionStart | `hooks.rs:189-192` | Resets `turn_count` and `current_turn_started_at` |
| PostToolUse compacting guard | `hooks.rs:428-458` | Does NOT override agent state if session is in "compacting" state |
| AgentState.state string values | `hooks.rs` throughout | 12 state strings: "thinking", "acting", "idle", "awaiting_input", "awaiting_approval", "needs_permission", "interrupted", "error", "delegating", "compacting", "task_complete", "session_ended" |
| Lazy session creation | `hooks.rs:118-175` | If hook arrives for unknown session, creates skeleton LiveSession |
| `hook-events-to-messages.ts` extra functions | Lines 14-92 | `hookEventsToMessages()` (→Message[]), `getMessageSortTs()`, `mergeByTimestamp()` — undocumented |
| `HookEvent.timestamp` is `bigint` | `packages/shared/src/types/generated/HookEvent.ts` | But `HookEventItem.timestamp` is `number` — conversion happens during mapping |

---

## Implementation Plan — 2 Tasks

### Task 1: Add Missing Tag-Stripping Regexes to Rust Parser

**Status: IN PROGRESS (partially implemented)**

**Files:**
- Modify: `crates/core/src/parser.rs`

**What was done:**
1. Added 2 new regexes at line ~78:
   ```rust
   let local_stdout_regex = Regex::new(r"(?s)<local-command-stdout>.*?</local-command-stdout>\s*").unwrap();
   let system_reminder_regex = Regex::new(r"(?s)<system-reminder>.*?</system-reminder>\s*").unwrap();
   ```
2. Updated `clean_command_tags()` function signature to accept 5 regex params (was 3)
3. Added 2 new `replace_all` calls in the stripping path
4. Updated all 3 call sites (lines ~166, ~191, ~214) to pass the new regexes
5. Refactored existing unit tests to use a `tag_regexes()` helper that returns all 5
6. Added 2 new tests:
   - `test_clean_command_tags_strips_local_stdout_and_system_reminder` — both tags
   - `test_clean_command_tags_strips_system_reminder_only` — single tag

**What still needs to be done:**
- Run `cargo test -p claude-view-core parser::tests::test_clean_command_tags` to verify all tests pass
- Commit

**Audit fix applied:** Call site 3 (`_` fallback/legacy arm, line 216) was missing `&local_stdout_regex, &system_reminder_regex` — would not compile. Fixed during audit.

**Test command:**
```bash
cargo test -p claude-view-core parser::tests::test_clean_command_tags
```

---

### Task 2: Fix `CATEGORY_BADGE` and `BADGE_LABELS` in ActionRow

**Status: NOT STARTED**

**Files:**
- Modify: `apps/web/src/components/live/action-log/ActionRow.tsx:6-29`

**Step 1: Add missing `CATEGORY_BADGE` entries**

At `ActionRow.tsx:6-17`, add after the `queue` entry:
```typescript
context: 'bg-emerald-500/10 text-emerald-400',
result: 'bg-green-500/10 text-green-400',
summary: 'bg-rose-500/10 text-rose-400',
```

Colors match their chip colors in `ActionFilterChips.tsx`.

**Step 2: Fix `BADGE_LABELS`**

At `ActionRow.tsx:19-29`, change `hook_progress` and add missing entries:
```typescript
hook_progress: 'Hook Progress',  // was "Hook" — now distinguishable from hook
context: 'Context',
result: 'Result',
summary: 'Summary',
```

**Step 3: Commit**

```bash
git add apps/web/src/components/live/action-log/ActionRow.tsx
git commit -m "fix(ui): add missing CATEGORY_BADGE and BADGE_LABELS for context, result, summary; disambiguate hook_progress"
```

---

## Verification Checklist

After all tasks complete:

1. **Rust tests:** `cargo test -p claude-view-core parser::tests::test_clean_command_tags`
2. **Frontend build:** `cd apps/web && bun run build` (no type errors)
3. **Visual check:** Open claude-view, load a session, toggle verbose mode:
   - All 13 filter chips render with correct colors and counts
   - Badge labels in ActionRow match chip labels (no raw category strings)
   - `hook` vs `hook_progress` are visually distinguishable
4. **Doc alignment:** Re-read doc §2.1 (command tags) and §6 (categories) — both should now match code

---

## Key Files Reference

| File | Purpose |
|------|---------|
| `docs/architecture/message-types.md` | The spec (source of truth) |
| `crates/core/src/parser.rs` | JSONL parser — 9 match arms + wildcard |
| `crates/core/src/types.rs` | Role enum (7), ContentBlock enum (5) |
| `crates/core/src/category.rs` | `categorize_tool()` + `categorize_progress()` |
| `crates/server/src/routes/hooks.rs` | Hook handler — 15 event names, agent state FSM |
| `apps/web/src/components/live/RichPane.tsx` | RichMessage interface + parseRichMessage() |
| `apps/web/src/components/live/action-log/ActionRow.tsx` | CATEGORY_BADGE + BADGE_LABELS maps |
| `apps/web/src/components/live/action-log/ActionFilterChips.tsx` | 13 category chips + "All" meta-chip |
| `apps/web/src/components/live/action-log/types.ts` | ActionCategory (13 variants), ActionItem, HookEventItem, TimelineItem |
| `apps/web/src/lib/message-to-rich.ts` | JSONL Message[] → RichMessage[] converter (7-role switch, hook_progress override) |
| `apps/web/src/lib/hook-events-to-messages.ts` | SQLite hook events → timeline items |
| `apps/web/src/lib/compute-category-counts.ts` | CategoryCounts utility |
| `packages/shared/src/types/generated/` | HookEvent, LiveSession, AgentState, AgentStateGroup, TokenUsage, SubAgentInfo, etc. |

---

## Git State Warning

The working tree has many pre-existing unstaged changes (see `git status` at session start). Per CLAUDE.md git discipline rules: **only commit files YOU changed**. The user's WIP is sacred.

Files modified by this session:
- `crates/core/src/parser.rs` — Task 1 changes (in progress)
- `apps/web/src/components/live/action-log/ActionRow.tsx` — Task 2 (not started)

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `clean_command_tags()` call site 3 (`_` wildcard/legacy arm, `parser.rs:216`) passed 4 args instead of 6 — won't compile | **Blocker** | Added missing `&local_stdout_regex, &system_reminder_regex` args to call site 3. Fixed in both `parser.rs` (code) and plan (documented). |
| 2 | Plan claimed hook_events DB schema has "7 columns" — actual is 8 (includes `id INTEGER PRIMARY KEY`) | Minor | Corrected plan line 36 to say "8 columns (incl. `id` PK)" |
