---
status: done
date: 2026-01-27
---

# DFS Path Resolution + Git Root Display Names

## Problem

Claude Code encodes project paths by replacing `/` with `-` and `/@` with `--`.
This encoding is ambiguous — literal hyphens in directory names look identical to
path separators. The current algorithm generates a fixed set of variants (tail-join
heuristics) that fails for:

- **`-Users-TBGor-dev--vicky-ai-claude-view`** → resolves to `/Users/TBGor/dev/@vicky/ai/claude/view` → display name "view" (should be "claude-view")
- **`-Users-TBGor-dev--vicky-ai-fluffy-web`** → resolves to `/Users/TBGor/dev/@vicky/ai/fluffy/web` → display name "web" (should be "fluffy/web")

Root cause: `--` is correctly converted to `/@`, but the remaining `-` in `vicky-ai`
is treated as a path separator because the algorithm only joins segments from the tail,
never in the middle.

## Solution

### 1. DFS Segment Walk (replaces `get_join_variants` + `resolve_project_path`)

Tokenize the encoded name, then walk left-to-right using depth-first search with
backtracking to find the actual filesystem path.

**Tokenization:**
1. Strip leading `-`
2. Replace `--` with `\0@` placeholder
3. Split on single `-`
4. Restore `\0@` → `@` prefix on segment

**DFS Walk:**
At each position, try:
1. Current segment as new directory → if exists as dir, recurse
2. Join current + next segment(s) with `-` → check again (lookahead cap: 4 segments)
3. Join with `.` instead of `-` (domain names like `Famatch.io`)
4. If at last segment(s), check for file/dir existence of joined path
5. Backtrack if no child leads to complete resolution

**Fallback:** If no path resolves on filesystem, use the first variant (all-separators)
as before.

### 2. Git Root Display Names (replaces `file_name()` extraction)

After resolving the full path, derive the display name from the nearest git root:

1. Walk up from resolved path checking for `.git` directory
2. Cap upward walk at 5 levels (monorepo safety)
3. `display_name = git_root_dirname / relative_path_to_resolved`

Examples:
| Resolved Path | Git Root | Display Name |
|---|---|---|
| `/Users/TBGor/dev/@vicky-ai/claude-view` | `claude-view/.git` | `claude-view` |
| `/Users/TBGor/dev/@vicky-ai/fluffy/web` | `fluffy/.git` | `fluffy/web` |
| `/Users/TBGor/dev/@Famatch.io` | `@Famatch.io/.git` | `@Famatch.io` |
| `/Users/TBGor` | (none) | `TBGor` |

Fallback: if no `.git` found, use last path component (current behavior).

## Files Changed

- `crates/core/src/discovery.rs` — replace resolution algorithm + add git root helper
- Tests updated in same file

## Non-Goals

- No frontend changes (backend already sends `display_name`)
- No API contract changes (`ResolvedProject` struct unchanged)
- No async changes (filesystem stat calls are fast, synchronous is fine)
