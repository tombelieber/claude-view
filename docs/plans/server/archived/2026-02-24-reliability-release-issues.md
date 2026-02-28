# Reliability Release — Issue Analysis

**Date:** 2026-02-24
**Status:** Issues documented, solutions TBD
**Priority:** HIGH — must ship before any new features (#7: reliability is paramount)
**Branch:** TBD (create new session to design solutions)

## Context

Feedback from company work machine (sandboxed corporate Mac) revealed four foundational issues that make claude-view untrustworthy in real-world environments. These must be fixed as a single cohesive reliability release before any new feature work.

---

## Issue 1: Path Config Scattered Across Codebase

**Symptom:** User runs in a corporate sandbox that can READ outside the sandbox boundary but CANNOT WRITE outside it. claude-view's write paths (SQLite DB, Tantivy index, etc.) are configured in ~10 different places across the codebase, making it impossible to redirect all writes to a sandbox-safe location.

**Environment:**
- Corporate Mac with sandbox security policy
- Sandbox CAN read `~/.claude/` (session JSONL files are accessible)
- Sandbox CANNOT write outside sandbox boundary
- claude-view needs to write its DB/index somewhere writable

**Root cause:** No single configuration root for all file paths. Write destinations are hardcoded or scattered across multiple crates/modules.

**Desired outcome:** One config point (env var or config file) that controls where claude-view writes all its data. All crates read from this single source.

---

## Issue 2: Hook Installation Requires Setup Script

**Symptom:** Because the sandbox cannot write to `~/.claude/settings.json`, claude-view cannot auto-install its hooks. Users need a manual setup step.

**Root cause:** Hook auto-install assumes write access to `~/.claude/`, which sandboxed environments don't have.

**Desired outcome:** A shell script (e.g., `claude-view setup-hooks`) that runs outside the sandbox to install necessary hooks into `~/.claude/settings.json`. Run once, then claude-view operates read-only from within the sandbox.

---

## Issue 3: Session Count Inflated (>> Real Main Sessions)

**Symptom:** App shows 1660 sessions on company machine, which is far more than the actual number of main (human-initiated) sessions.

**Root cause found:** `discover_orphan_sessions()` in `crates/core/src/session_index.rs:175` treats every `.jsonl` file in project directories without a `sessions-index.json` as a session. This includes:
- `file-history-snapshot` files (929 on dev machine)
- `queue-operation` files (434) — subagent task queues
- `progress` files (134)
- `summary` files (23)
- Continuation sessions with `parentUuid` (444)

**Data from dev machine:**
```
Total JSONL files on disk:              4146
Sessions in sessions-index.json:         713 (all isSidechain=false)
file-history-snapshot (not sessions):    929
queue-operation (subagent tasks):        434
progress files:                          134
summary files:                            23
user-started, no parent (REAL MAIN):    2122
user-started, has parent (continues):    444
assistant-started:                        60
```

**Key code location:** `crates/core/src/session_index.rs:175` — `discover_orphan_sessions()`

**Desired outcome:** Orphan discovery filters out non-conversation JSONL files. Only files whose first line is `type: "user"` with no `parentUuid` should count as main sessions.

**Note on `sessions-index.json`:** Do NOT trust this as a reliable source. It has had bugs and feels like a legacy artifact from Claude Code. Use it as a hint, not as ground truth. The filesystem + JSONL first-line peek is the authoritative source.

---

## Issue 4: Project Path Resolve Shows Wrong Names

**Symptom:** On company machine, project names resolve incorrectly. For example, `claude-view` shows as just `view`. The encoded directory name `-Users-user-dev-claude-view` gets tokenized into segments `["Users", "user", "dev", "claude", "view"]`, DFS resolve fails (path doesn't exist or different structure), fallback kicks in and joins all segments as `/Users/user/dev/claude/view`, then `derive_display_name` takes the last component → `view`.

**Root cause:** Two problems:
1. DFS resolve fails on company machine (need to debug why — the sandbox CAN read the filesystem)
2. The fallback `format!("/{}", segments.join("/"))` silently produces wrong results instead of surfacing an error

**Key code locations:**
- `crates/core/src/discovery.rs:60` — `resolve_project_path()`
- `crates/core/src/discovery.rs:78` — DFS resolve + fallback
- `crates/core/src/discovery.rs:116` — `tokenize_encoded_name()`
- `crates/core/src/discovery.rs:162` — `dfs_resolve()`
- `crates/core/src/discovery.rs:259` — `derive_display_name()`

**Desired outcome:**
1. Kill the fallback entirely. If DFS can't resolve, show "unresolved" — never guess. Wrong data is worse than missing data.
2. Debug why DFS fails on company machine (the filesystem is readable, so `read_dir` should work)
3. Where possible, use `projectPath` from the JSONL session data itself (which Claude Code writes with the real path) rather than reverse-engineering from the encoded directory name

---

## Design Principles for Solutions

1. **Show errors, not guesses.** If data can't be resolved, surface it explicitly. Never silently produce wrong results.
2. **Don't trust external sources blindly.** `sessions-index.json` is a hint, not ground truth. Cross-check against filesystem.
3. **One config root.** All path configuration flows from a single source.
4. **Sandbox-compatible by default.** Read from anywhere, write to a configurable location.

---

## Competitor Context

Both major competitors (jhlee0409/claude-code-history-viewer, d-kimuson/claude-code-viewer) have NO persistent database, NO path resolution logic, and NO sandbox support. They re-parse JSONL on every request. Our SQLite + Tantivy architecture is fundamentally superior but introduces these configuration challenges. Fixing them solidifies our architectural advantage.

---

## Next Steps

- [ ] Create a new session to design solutions for each issue
- [ ] Write implementation plan after design approval
- [ ] Ship as a single reliability release before any new features
