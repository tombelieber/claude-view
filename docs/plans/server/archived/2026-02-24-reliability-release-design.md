# Reliability Release — Design

**Date:** 2026-02-24
**Status:** Approved
**Priority:** HIGH — must ship before any new features
**Depends on:** `2026-02-24-reliability-release-issues.md` (issue analysis)

## Context

Feedback from a corporate Mac running DataCloak (kernel-level DLP sandbox) revealed four foundational issues. This design fixes all four as a single cohesive reliability release. Every decision below is backed by empirical data from a full audit of 4,159 JSONL files across 634,424 lines.

---

## Issue 1: Single Config Root for All Write Paths

### Problem

All write paths derive from `dirs::cache_dir()` with no override. DataCloak sandbox blocks writes to `~/Library/Caches/`. The app can't start.

### Design

**One env var: `CLAUDE_VIEW_DATA_DIR`**

Two modes, zero config friction:

| Mode | Who | How data dir is resolved |
|------|-----|--------------------------|
| **Dev/demo** (`bun dev`) | Developer inside DataCloak | `.env` sets `CLAUDE_VIEW_DATA_DIR=./.data` — everything stays in repo |
| **Production** (`npx claude-view`) | End users | No `.env` exists → falls back to `~/Library/Caches/claude-view/` |

**ALL writes go to one directory. No exceptions.**

```
data_dir()/                        # ~/Library/Caches/claude-view/ OR ./.data/
├── claude-view.db                 # SQLite database
├── search-index/                  # Tantivy index
└── locks/                         # Lock files (moved from /tmp/)
```

No writes to `~/.claude/`. No writes to `/tmp/`. No writes anywhere else.

**Clean uninstall:**
- Production users: `rm -rf ~/Library/Caches/claude-view`
- Dev/demo: `rm -rf .data`

### Implementation

**`crates/core/src/paths.rs`** — new top-level function:

```rust
pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CLAUDE_VIEW_DATA_DIR") {
        let path = PathBuf::from(&dir);
        if path.is_relative() {
            std::env::current_dir().unwrap().join(path)
        } else {
            path
        }
    } else {
        dirs::cache_dir()
            .map(|d| d.join("claude-view"))
            .expect("no cache dir found")
    }
}
```

All existing path functions derive from `data_dir()`:
- `db_path()` → `data_dir().join("claude-view.db")`
- `search_index_dir()` → `data_dir().join("search-index")`
- Lock files → `data_dir().join("locks/")` (move out of `/tmp/`)

**Startup validation** (in `main.rs`, before DB/index init):
1. `create_dir_all(data_dir())`
2. Attempt to create+delete a temp file
3. If either fails → print: `"Cannot write to {path}. Set CLAUDE_VIEW_DATA_DIR to a writable directory."` → exit non-zero

**Repo files:**
- `.env.example` (committed): `CLAUDE_VIEW_DATA_DIR=./.data`
- `.env` (gitignored): user copies from `.env.example`
- `.data/` (gitignored)

**Proven pattern:** Docker (`DOCKER_DATA_ROOT`), Homebrew (`HOMEBREW_PREFIX`), Cargo (`CARGO_HOME`), Go (`GOPATH`).

---

## Issue 2: Hook Installation

### Problem

DataCloak sandbox can't write to `~/.claude/settings.json` for hook auto-install.

### Design

**Docs-only fix.** Add a copy-paste shell command to README that users run once outside the sandbox. No CLI subcommand needed — YAGNI.

---

## Issue 3: Session Count — Full Topology Graph

### Problem

App shows 1,660 sessions when only ~700 are real main sessions. `discover_orphan_sessions()` counts every `.jsonl` file as a session.

### Empirical Audit Results

Full audit of 4,159 JSONL files, 634,424 lines, 0 parse errors:

**First-line type distribution:**

| First-line type | Count | What they are |
|-----------------|-------|---------------|
| `user` | 2,574 | Sessions (normal start) |
| `file-history-snapshot` | 928 | Sessions (resumed with history restore) |
| `queue-operation` | 430 | Sessions (queued/resumed) |
| `progress` | 134 | Sessions (progress preamble) |
| `assistant` | 60 | Subagent transcripts (all at depth 3) |
| `summary` | 23 | Sessions (continuation with summary) |

**94 unique type signatures** across files. Dominant: `user -> assistant -> progress`.

**Directory depth:**
- Depth 1 = project-level files (sessions + metadata)
- Depth 3 = `{sessionId}/subagents/agent-{agentId}.jsonl` (subagent transcripts)

**`cwd` field presence:** 4,114 / 4,159 (98.9%) have `cwd` within first 10 lines. The 45 without are all metadata files (file-history-snapshot: 27, summary: 11, queue-operation: 7). Zero conversation sessions missing `cwd`.

**Linking mechanisms:**

| File type | Count | Linking field | Resolution |
|-----------|-------|--------------|------------|
| Sessions (user-started) | ~2,574 | filename = sessionId | Self |
| Subagent transcripts | 2,619 | parent dir name = sessionId | Path-based |
| queue-operation | 430 | `sessionId` field, filename matches | Direct |
| progress | 134 | `sessionId` field, filename matches | Direct |
| file-history-snapshot | 929 | `messageId` only | UUID cross-ref during indexing |
| summary | 23 | `leafUuid` only | UUID cross-ref during indexing |

### Design

**Parse everything, hide nothing, model the full relationship graph.**

A file is a **main session** if:
1. Contains at least one `"type":"user"` AND one `"type":"assistant"` line (conversation happened)
2. First `user` line has no `parentUuid` (not a fork/continuation)

Scanning uses existing `memmem::Finder` SIMD pre-filter — scan for `"type":"user"` and `"type":"assistant"` byte patterns, only JSON-parse matching lines. Stop once both conditions resolved. For 62% of files (user-started), this resolves in 2 lines.

**Data model:**

```rust
Session {
    id: SessionId,                    // from filename
    kind: Conversation | MetadataOnly,
    start_type: User | FileHistorySnapshot | QueueOperation | Progress | Summary,
    parent_id: Option<SessionId>,     // from parentUuid on first user line
    project: ProjectId,
    cwd: Option<String>,              // from JSONL, authoritative project path
}

SubagentRef {
    agent_id: String,                 // from filename agent-{agentId}.jsonl
    session_id: SessionId,            // from parent directory name
}

MetadataFile {
    file_path: PathBuf,
    kind: FileHistorySnapshot | Summary,
    linking_uuid: String,             // messageId or leafUuid
    linked_session_id: Option<SessionId>, // resolved during indexing
}
```

**Indexing (three passes):**

1. **Pass 1 — Scan depth-1 `.jsonl` files:** Extract sessionId (filename), cwd, parentUuid, start_type. Build UUID lookup: `message_uuid → sessionId`.
2. **Pass 2 — Walk depth-3 subagent directories:** Link via parent directory name.
3. **Pass 3 — Resolve orphans:** Look up `messageId`/`leafUuid` in UUID table. Unresolved → store as unlinked (don't hide, don't guess).

**UI top-level list:** `kind=Conversation AND parent_id IS NULL` = main sessions. Everything else via drill-down.

**Session count: provably correct.** ~2,130 main sessions, not 1,660.

---

## Issue 4: Project Path Resolution

### Problem

Project names resolve wrong (`claude-view` → `view`). DFS filesystem walk fails in DataCloak sandbox, fallback silently guesses wrong path.

### Empirical Finding

`cwd` field is present in 98.9% of all JSONL files (4,114 / 4,159). The 45 files without `cwd` are all metadata files (not conversations). **Zero conversation sessions are missing `cwd`.**

### Design

**Use `cwd` from JSONL as the sole source of truth. Delete DFS resolve.**

```
Conversation sessions → cwd from JSONL → done
Metadata files (45)  → linked to parent session via UUID → inherit cwd from parent
```

**Delete:**
- `dfs_resolve()` — dead code, never needed
- `tokenize_encoded_name()` — dead code
- Silent fallback `format!("/{}", segments.join("/"))` — the root cause of wrong names
- Entire reverse-engineering approach of encoded directory names

**Keep:**
- `derive_display_name(path)` — still useful, but now receives the REAL path from `cwd` instead of a guessed one
- Walk up to find nearest `.git` root for display name

**For projects where `cwd` path doesn't exist on current machine** (e.g., viewing sessions from a different machine): show the raw `cwd` value. It's the real path — just not local. Still infinitely better than guessing.

**Proven principle:** Show errors, not guesses. The `cwd` field is written by Claude Code itself — it's the canonical source.

---

## Design Principles

1. **Show errors, not guesses.** If data can't be resolved, surface it explicitly. Never silently produce wrong results.
2. **One config root.** All write paths flow from `CLAUDE_VIEW_DATA_DIR`.
3. **Parse everything, hide nothing.** Build the complete session topology. Let the UI decide what to show.
4. **Source of truth from JSONL.** `cwd` for project paths, content scanning for session classification. Don't reverse-engineer what's already in the data.
5. **Sandbox-proof by design.** Read from `~/.claude/`, write to `data_dir()`, path resolution from file content not filesystem walking.

---

## Competitor Context

Both major competitors (jhlee0409/claude-code-history-viewer, d-kimuson/claude-code-viewer) have NO persistent database, NO session topology, NO sandbox support, and NO path resolution. They re-parse JSONL on every request and show a flat list. This reliability release solidifies our architectural advantage with:
- Full session relationship graph (forks, subagents, metadata linking)
- Enterprise sandbox compatibility out of the box
- Provably correct session counts backed by empirical data
- One-command data management (single config root)
