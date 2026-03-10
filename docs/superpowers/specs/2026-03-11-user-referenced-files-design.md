# User-Referenced Files on Live Session Cards — Design Spec

> **Status:** APPROVED
> **Date:** 2026-03-11

## Problem

Live session cards show project name, branch, cost, and agent state — but not **what task/feature/plan the session is working on**. Users running multiple parallel sessions can't distinguish "which session is doing what" without clicking into each one.

## Solution

Extract `@file` mentions from user messages, accumulate them as a deduplicated set across the session lifetime, and display them as file chips on the live session card.

## Design Decisions (Evidence-Backed)

### Why `@` mentions only (not pasted file paths)

**Data:** Full corpus scan of 407 sessions (3,602 user messages):

- `@` mention regex: 127 matches, 43 unique files, **0 false positives** (after `@@` exclusion)
- Pasted path regex: high noise — skill prompts, system tags, example code all contain file paths that aren't user intent
- `@` mentions are explicitly intentional (user typed `@` + selected a file), pasted paths are ambiguous

**Industry precedent:** GitHub, Slack, Linear, Notion — all use explicit prefix sigils for structured extraction from freeform text.

### Why accumulated set (not latest-only)

The first `@` mention typically defines the session's identity (e.g., `@docs/plans/2026-03-07-unified-notification-system.md`). Replacing it with later mentions would lose the session identification signal.

### Why backend extraction (not frontend regex)

Follows existing patterns (`tool_names`, `skill_names`, `ide_file` all extracted in `live_parser.rs`). Single extraction point serves all consumers (SSE, API, future CLI).

### `user_files` vs `last_user_file` — independent fields

`last_user_file: Option<String>` (existing) captures the single `<ide_opened_file>` tag pushed by the IDE integration. `user_files: Option<Vec<String>>` (new) captures explicit `@` mentions from user prompts. These are independent data sources with different semantics:

- **`last_user_file`** — IDE push (automatic, reflects editor state)
- **`user_files`** — User intent (explicit `@` reference in prompt text)

They remain separate fields. The frontend merges them visually in a single "file context" row with deduplication.

## Data Flow

```text
User message content
  → live_parser.rs: extract @file mentions from STRIPPED content
      (inside extract_content_and_tools(), applied to local `stripped` variable
       returned by strip_noise_tags() — NOT a separate pre-pass)
  → LiveLine.at_files: Vec<String>
  → manager.rs SessionAccumulator.at_files: HashSet<String> (deduped, ≤10, first-N-wins)
  → JsonlMetadata.user_files: Option<Vec<String>>
  → apply_jsonl_metadata() → LiveSession.user_files: Option<Vec<String>>
  → SSE JSON: "userFiles": ["docs/plans/foo.md", "src/auth.ts"]  (camelCase via serde)
  → Generated TS (packages/shared): userFiles?: Array<string> | null
  → Hand-written LiveSession in use-live-sessions.ts: userFiles?: string[] | null
  → SessionCard: file chips (≤3 visible, +N overflow)
```

## Implementation Details

### 1. Parser (`crates/core/src/live_parser.rs`)

**Regex:** `(?:^|\s)@([\w./-]+\.\w{1,15})`

- Matches `@docs/plans/foo.md`, `@src/auth.ts`, `@README.md`
- Rejects `@claude-view/plugin@0.11.0` (contains second `@` in match — excluded by `[\w./-]` not containing `@`)
- Extension cap `\w{1,15}` accommodates `.typescript` (10 chars) and similar long extensions
- Runs on the original string `s` (or `text` in the array branch) inside `extract_content_and_tools()`, **before** `truncate_str()` consumes the stripped content. Critical ordering: `strip_noise_tags()` returns `(stripped, ide_file)`, then `truncate_str(&stripped, 200)` moves `stripped` — so `@` extraction must happen on `s` (unstripped) or on a `&stripped` borrow before the move. Using `s` is simpler and correct since user-authored `@` mentions are never inside system tags.

**SIMD pre-filter:** Add `at_file_key: memmem::Finder<'static>` to `TailFinders` struct (line ~166) initialized with `memmem::Finder::new(b"@")`. Use bare `b"@"` (not `b" @"`) because messages can begin with `@file` at position 0 — the regex `(?:^|\s)@` handles start-of-string, but a `b" @"` pre-filter would miss it. Gate the regex behind `finders.at_file_key.find(raw).is_some()`.

**Return type change:** `extract_content_and_tools()` currently returns a 5-tuple `(String, Vec<String>, Vec<String>, bool, Option<String>)`. Adding `at_files` requires expanding to a 6-tuple: `(String, Vec<String>, Vec<String>, bool, Option<String>, Vec<String>)` — or refactoring to a named struct. Since there is only one call site (`parse_single_line` at line ~507), the 6-tuple approach is acceptable for now. Update the destructuring at the call site.

**New field on `LiveLine`:**

```rust
pub at_files: Vec<String>,
```

**Mandatory update sites** (Rust struct literals require all fields):

- `parse_single_line()` error-path literal (line ~441): add `at_files: Vec::new()`
- `empty_line()` test helper in `accumulator.rs` (line ~614): add `at_files: Vec::new()`

### 2. Accumulator (`crates/server/src/live/manager.rs` — NOT `crates/core/src/accumulator.rs`)

**Important:** There are TWO `SessionAccumulator` structs in the codebase. The core one (`crates/core/src/accumulator.rs`) is for history batch parsing. The live path exclusively uses the **private** `SessionAccumulator` in `crates/server/src/live/manager.rs` (line ~38). The new field goes on the manager's accumulator.

**New field on manager's `SessionAccumulator`:**

```rust
pub at_files: HashSet<String>,
```

Uses `HashSet<String>` (not `BTreeSet`) to match the existing pattern — `mcp_servers` and `skills` both use `HashSet<String>`, with sorting applied at collection time when building `JsonlMetadata`.

**Cap semantics — first-N-wins:**

```rust
for file in line.at_files.iter() {
    if acc.at_files.len() < 10 {
        acc.at_files.insert(file.clone());
    }
}
```

Once 10 unique files are accumulated, new mentions are silently dropped. This bounds memory and preserves the earliest mentions (which typically define session identity).

**File-replacement clear:** When a session file is replaced (detected by `offset > file_len`), clear `at_files` alongside `mcp_servers` and `skills` at line ~1512:

```rust
acc.at_files.clear();
```

**Import:** Add `HashSet` to the existing `use std::collections::HashMap` import (or use inline `std::collections::HashSet` as done elsewhere in the file).

### 3. `JsonlMetadata` intermediary (`crates/server/src/live/manager.rs`)

**New field on `JsonlMetadata` (line ~144):**

```rust
user_files: Option<Vec<String>>,
```

**Population** (three `JsonlMetadata { ... }` construction sites: the literal inside `enrich_session_from_accumulator()` at line ~496, the recovery path at line ~827, and the `process_jsonl_update` path at line ~2013):

```rust
user_files: if acc.at_files.is_empty() {
    None
} else {
    let mut files: Vec<String> = acc.at_files.iter().cloned().collect();
    files.sort();
    Some(files)
},
```

Sort at collection time for deterministic ordering (matches `mcp_servers`/`skills` pattern).

### 4. `apply_jsonl_metadata()` (`crates/server/src/live/manager.rs`, line ~304)

Wire `JsonlMetadata.user_files` → `LiveSession.user_files`:

```rust
if m.user_files.is_some() {
    session.user_files = m.user_files.clone();
}
```

Note: `apply_jsonl_metadata` takes `m: &JsonlMetadata` (shared reference). You cannot move `Vec<String>` out of a `&T` — use `.clone()` as done for `last_user_file` at line ~331.

### 5. State (`crates/server/src/live/state.rs`)

**New field on `LiveSession` (line ~95):**

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub user_files: Option<Vec<String>>,
```

- `#[serde(default)]` — makes ts-rs 11 generate the field as optional (`userFiles?:` not `userFiles:`)
- `#[serde(skip_serializing_if = "Option::is_none")]` — omits field from JSON when `None` (keeps SSE payload small for 87% of sessions without `@` mentions). Without this, `None` serializes as `"userFiles": null`
- Combined effect: ts-rs generates `userFiles?: Array<string> | null` — matching the established pattern from `ProgressItem.id` and `SubAgentInfo.agent_id`
- `#[serde(rename_all = "camelCase")]` on the struct handles `user_files` → `userFiles` automatically
- Note: `#[ts(optional)]` does NOT exist in ts-rs 11 — do not use it

**Mandatory update sites** (struct literal construction):

- `build_recovered_session()` (line ~173): add `user_files: None`
- `minimal_live_session()` test helper in `state.rs` (line ~505): add `user_files: None`
- All `LiveSession { ... }` construction sites in `manager.rs` (~4 sites): add `user_files: None`

### 6. Frontend Types

**Generated type** (`packages/shared/src/types/generated/LiveSession.ts`):
After adding the Rust field and running codegen, this file will automatically include `userFiles?: Array<string> | null`.

**Hand-written type** (`apps/web/src/components/live/use-live-sessions.ts`, line ~10):
The `SessionCard` component imports `LiveSession` from this hand-written interface, NOT from the generated type. Add:

```typescript
userFiles?: string[] | null;
```

after the existing `lastUserFile` field (line ~25).

> **Tech debt note:** This hand-written `LiveSession` duplicates the generated type — a CLAUDE.md hard-rule violation. Long-term: replace with the generated `LiveSession` from `@claude-view/shared`. Out of scope for this feature.

### 7. Frontend Display (`apps/web/src/components/live/SessionCard.tsx`)

**Restructure the existing IDE chip block** (lines ~208–215). The current `lastUserFile` chip renders in its own `<div>`. Replace with a unified "file context" row that merges both `@` mention chips and the IDE file chip.

**Display rules:**

- Single row containing all file chips (both `@` mentions and IDE file)
- `@` mention chips: emerald/teal styling (`bg-emerald-50 dark:bg-emerald-950/40 border-emerald-200 dark:border-emerald-800 text-emerald-700 dark:text-emerald-300`) with `@` prefix icon
- IDE file chip: existing sky-blue styling (unchanged)
- Show filename only (`path.split('/').pop()`), full path in `title` tooltip (add `title={fullPath}` to BOTH chip types — the existing IDE chip is missing this)
- Max 3 chips visible total, `+N more` overflow with Radix Tooltip. Define `const MAX_VISIBLE_FILES = 3`. Reference pattern: `SessionToolChips.tsx` uses `MAX_VISIBLE = 4` with Radix Tooltip overflow; `SubAgentPills.tsx` uses hardcoded `slice(0, 3)`. Follow the `SessionToolChips` named-constant approach but with value 3 for file chips.
- **Deduplication:** If `lastUserFile` path appears in `userFiles` set, show only the `@` mention chip (emerald) — don't render a duplicate sky-blue chip
- Absolute paths (`@/Users/.../foo.md`): strip to relative from project root if possible, else show filename only

### 8. `SessionPanelData` — Explicit Exclusion (for now)

File chips are **card-only** for this feature. `SessionPanelData` (`session-panel-data.ts`) does NOT get a `userFiles` field. The detail panel does not show file chips.

**Rationale:** The detail panel shows the full conversation — the user can see their own `@` mentions inline. Adding redundant chips adds no value there. If this changes, both `liveSessionToPanelData()` and `historyToPanelData()` must be updated per the CLAUDE.md parity rule.

> **Pre-existing gap:** `lastUserFile` is also absent from `SessionPanelData`. This is a separate issue unrelated to this feature.

### What we're NOT doing

- Pasted file path extraction (too noisy, 0% precision without `@` prefix)
- Agent tool_use file paths (deferred — verbose/debug mode feature)
- History session support (live monitor only for now)
- New API endpoint (data flows through existing SSE)
- Replacing the hand-written `LiveSession` TS type with the generated one (tech debt, separate task)

## Regex Validation

Tested against full corpus (407 sessions, 3,602 user messages):

| Metric | Value |
| --- | --- |
| Sessions with `@` refs | 52 (12.8%) |
| Total matches | 127 |
| True positives | 124 |
| False positives (pre-fix) | 3 (`@scope@version` npm refs) |
| False positives (post-fix) | 0 |
| Precision | 100% |

Top matches confirm signal quality:

- `@docs/plans/2026-02-28-smart-search-implementation.md` (14x)
- `@docs/plans/2026-02-28-smart-search-design.md` (13x)
- `@docs/plans/2026-03-07-unified-notification-system.md` (6x)

## Rollback

All changes are additive — new fields with `Option`/`Vec::new()` defaults. To roll back:

1. Remove the field from `LiveSession` + all construction sites
2. Remove accumulator field + `JsonlMetadata` field + `apply_jsonl_metadata` wiring
3. Remove `at_files` from `LiveLine` + extraction logic
4. Revert `SessionCard.tsx` to the previous `lastUserFile`-only block
5. Remove `userFiles` from the hand-written `LiveSession` TS interface

No database migrations, no schema changes, no external API changes.

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
| --- | --- | --- | --- |
| 1 | Spec said `LiveSessionState` — struct is actually `LiveSession` | Blocker | Renamed throughout |
| 2 | Spec targeted `crates/core/src/accumulator.rs` — live path uses manager's private `SessionAccumulator` | Blocker | Redirected to `crates/server/src/live/manager.rs` |
| 3 | Referenced phantom type `LiveSessionUpdate` — does not exist | Blocker | Replaced with actual path: `SessionAccumulator → JsonlMetadata → apply_jsonl_metadata() → LiveSession` |
| 4 | Missing `JsonlMetadata` intermediary wiring | Blocker | Added explicit Step 3 (JsonlMetadata) and Step 4 (apply_jsonl_metadata) |
| 5 | Missing `#[serde(skip_serializing_if)]` — `None` would serialize as `null` not omission | Blocker | Added both `skip_serializing_if` and `#[ts(optional)]` annotations |
| 6 | `extract_content_and_tools()` returns 5-tuple — adding field requires 6-tuple | Blocker | Documented return type expansion and call site update |
| 7 | Hand-written `LiveSession` in `use-live-sessions.ts` is what `SessionCard` consumes | Blocker | Added Step 6 for hand-written type update |
| 8 | Two `LiveLine` struct literals would fail to compile without new field | Warning | Documented mandatory update sites (error path + test helper) |
| 9 | No `memmem::Finder` pre-filter for `@` — inconsistent with SIMD-first convention | Warning | Added `at_file_key` finder to `TailFinders` |
| 10 | Existing accumulator sets use `HashSet`, not `BTreeSet` | Warning | Changed to `HashSet` with sort-at-collection-time |
| 11 | Cap semantics undefined | Warning | Specified "first-N-wins" with code example |
| 12 | 4+ `LiveSession` construction sites need `user_files: None` | Warning | Enumerated all sites |
| 13 | File-replacement clear block should clear `at_files` | Warning | Added explicit `acc.at_files.clear()` |
| 14 | Existing IDE chip missing `title` tooltip | Warning | Added to display rules |
| 15 | IDE chip block must be restructured for merged row | Warning | Documented restructure approach |
| 16 | Extension cap `\w{1,10}` rejects extensions > 10 chars | Minor | Widened to `\w{1,15}` |
| 17 | `last_user_file` vs `user_files` relationship undocumented | Minor | Added design decision section |
| 18 | `SessionPanelData` needs explicit decision re: parity | Minor | Added Step 8 with explicit exclusion rationale |
| 19 | `lastUserFile` absent from `SessionPanelData` | Minor | Noted as pre-existing gap |
| 20 | `apply_jsonl_metadata` code moves `Vec` out of `&T` — won't compile | Critical | Changed to `.clone()` pattern matching `last_user_file` at line ~331 |
| 21 | SIMD pre-filter `b" @"` misses `^@` messages at position 0 | Important | Changed to `b"@"` — regex handles false-positive filtering |
| 22 | `stripped` variable consumed by `truncate_str` before `@` extraction | Important | Changed to extract from `s` (unstripped) before move |
| 23 | `SessionToolChips` uses `MAX_VISIBLE = 4`, not 3; `SubAgentPills` has no constant | Important | Corrected reference pattern and defined `MAX_VISIBLE_FILES = 3` |
| 24 | `JsonlMetadata` construction site lines wrong (~851/~2036 → ~827/~2013) | Minor | Corrected line numbers |
| 25 | `minimal_live_session()` file not specified | Minor | Added `state.rs` file attribution |
| 26 | `#[ts(optional)]` does not exist in ts-rs 11 — would fail to compile | Critical | Replaced with `#[serde(default, skip_serializing_if)]` which is the actual mechanism for optional TS fields. Corrected generated type from `string[]` to `Array<string> \| null` |
