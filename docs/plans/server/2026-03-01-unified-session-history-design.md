# Unified Session History — Design

**Date:** 2026-03-01
**Status:** Approved
**Scope:** `crates/core`, `crates/db`, `crates/server`, `packages/shared`, `apps/web`

## Problem

Claude Code deletes sessions after 30 days. Users who run `claude-backup` have their full history preserved as `.jsonl.gz` files in `~/.claude-backup/`. But claude-view only reads from `~/.claude/projects/` — so users see a shrinking window of history, not their full archive.

The goal: **one unified timeline** combining live sessions (`~/.claude/`) and archived sessions (`~/.claude-backup/`), with zero configuration.

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Architecture | Pluggable `SessionSource` trait | Future providers (opencode, codex) add a new impl, zero indexer changes |
| Day 1 scope | `LiveSource` + `ClaudeBackupSource` only | Ship what we control |
| Dedup | Live wins — skip backup if UUID already indexed from live | Live is always more recent (actively appended) |
| Index timing | Startup, same pass as live | Full-text search works across all sessions immediately |
| UI treatment | Subtle "archived" badge on backup-only sessions | No separate tab, no filter — one unified list |
| Gzip handling | Decompress in-memory at index time only | SQLite + Tantivy serve all queries after indexing |
| Configuration | Auto-detect `~/.claude-backup/manifest.json` | Zero config for users; no backup = no change |
| Changes to claude-backup | None | Backup stays as-is; claude-view adapts |

## Verified Facts

| | Live (`~/.claude/projects/`) | Backup (`~/.claude-backup/machines/`) |
|---|---|---|
| Format | `{uuid}.jsonl` (raw) | `{uuid}.jsonl.gz` (gzipped) |
| Dir layout | `{projectHash}/{uuid}.jsonl` | `{machine}/projects/{projectHash}/{uuid}.jsonl.gz` |
| Content | Raw JSONL | Byte-identical when decompressed (verified) |
| Index file | `sessions-index.json` per project | `session-index.json` (flat, at root) |
| Overlap | Same UUIDs can appear in both sources |

## Architecture

### SessionSource Trait

New file: `crates/core/src/session_source.rs`

```rust
pub enum SessionSourceTag {
    Live,
    Archived,
}

pub struct DiscoveredSession {
    pub uuid: String,
    pub project_hash: String,
    pub file_path: PathBuf,
    pub file_size: u64,
    pub file_mtime: SystemTime,
    pub source_tag: SessionSourceTag,
}

pub trait SessionSource: Send + Sync {
    fn name(&self) -> &str;
    fn discover(&self) -> Result<Vec<DiscoveredSession>>;
    fn read_bytes(&self, session: &DiscoveredSession) -> Result<Vec<u8>>;
}
```

### LiveSource

Extracts current logic from `scan_and_index_all()`:
- Discovery: walk `~/.claude/projects/{hash}/{uuid}.jsonl`
- Read: mmap (>64KB) or `fs::read` (<64KB) — existing behavior, moved into trait impl

### ClaudeBackupSource

- Detection: check `~/.claude-backup/manifest.json` exists + valid JSON
- Discovery: walk `~/.claude-backup/machines/*/projects/{hash}/{uuid}.jsonl.gz`
- Read: `flate2::read::GzDecoder` → `Vec<u8>`
- If `~/.claude-backup/` doesn't exist → source returns empty vec, zero impact

### Dedup Flow

```
1. LiveSource.discover()    → 5,249 sessions
2. Build HashSet<UUID>      → 5,249 entries
3. BackupSource.discover()  → 1,785 sessions
4. Filter: skip if UUID in HashSet → ~600 backup-only remain
5. Merge into single Vec<DiscoveredSession>
6. Staleness check (size + mtime + parse_version) → skip unchanged
7. Parse only new/changed sessions
```

Source order is the contract: sources[0] always wins on UUID collision.

### Gzip Decompression

Only at `ClaudeBackupSource::read_bytes()`. The indexer receives `Vec<u8>` — identical bytes regardless of source. Everything downstream (SIMD pre-filter, line split, SQLite, Tantivy) is unchanged.

No mmap for `.gz` files (gzip is sequential, can't random-access). This is fine — backup sessions are indexed once, served from SQLite/Tantivy forever.

Staleness for `.gz` files: track the `.gz` file's `(size, mtime)` in SQLite. Backup re-sync changes mtime → re-index triggers.

**Precedent:** Same pattern as Git packfiles — compressed on disk, indexed once, queries served from the index.

## Schema Change

One new column on `sessions`:

```sql
ALTER TABLE sessions ADD COLUMN source TEXT NOT NULL DEFAULT 'live';
-- values: 'live' | 'archived'
```

Set during `write_results_sqlx()` from `DiscoveredSession.source_tag`.

Pruning safety: stale-session pruner checks if `file_path` exists on disk. For backup sessions, `file_path` points to the `.jsonl.gz`. If user deletes backup → sessions get pruned naturally.

## API Change

`GET /api/sessions` — add `source` field to response:

```json
{ "id": "abc-123", "source": "archived", ... }
```

No new endpoints. No breaking changes. Clients that don't read `source` behave identically.

## Frontend

`SessionCard.tsx` and `CompactSessionTable.tsx` — subtle inline badge:

```tsx
{session.source === 'archived' && (
  <span className="text-xs text-zinc-400 dark:text-zinc-500 ml-1.5"
        title="From backup archive">
    archived
  </span>
)}
```

No separate tab. No filter toggle. One unified list.

## Shared Types

`packages/shared/src/types/` — add `source?: 'live' | 'archived'` to session TS type. Optional for backwards compat.

## Startup Flow

Current:
```
bind port → resolve ~/.claude → build hints → scan_and_index_all(claude_dir) → prune
```

New:
```
bind port → resolve ~/.claude → build sources vec → build hints → scan_and_index_all(sources) → prune
```

```rust
let mut sources: Vec<Box<dyn SessionSource>> = vec![
    Box::new(LiveSource::new(&claude_dir)),
];
if let Ok(backup) = ClaudeBackupSource::detect() {
    info!("claude-backup detected, {} archived sessions", backup.session_count());
    sources.push(Box::new(backup));
}
```

120s periodic re-scan: same change. New backup sessions from daily sync picked up within 2 minutes.

## Performance Budget

| Operation | Cost | When |
|---|---|---|
| Backup discovery (walk dirs) | ~50ms for 1,785 files | Every re-scan |
| Dedup (HashSet lookup) | negligible | Every re-scan |
| Gzip decompress + parse | ~1-2s per session (avg 1.7MB) | Once per session, ever |
| First-time full backup index | ~30-60s for ~600 backup-only sessions | Once, then incremental |
| Subsequent re-scans | ~100ms (staleness skip) | Every 120s |

Server port is bound before indexing — no UI blocking.

## New Dependencies

| Crate | Where | Purpose |
|---|---|---|
| `flate2` | `crates/core/Cargo.toml` | Gzip decompression |

## Files Changed

| File | Change |
|---|---|
| **NEW** `crates/core/src/session_source.rs` | Trait + `LiveSource` + `ClaudeBackupSource` |
| `crates/core/src/lib.rs` | `pub mod session_source` |
| `crates/core/Cargo.toml` | Add `flate2` |
| `crates/db/src/indexer_parallel.rs` | `scan_and_index_all` takes `&[Box<dyn SessionSource>]`; extract file-reading into `LiveSource::read_bytes`; dedup HashSet |
| `crates/db/src/lib.rs` | Add `source TEXT` column + migration |
| `crates/server/src/main.rs` | Build sources vec, pass to indexer |
| `packages/shared/src/types/` | Add `source` field to session TS type |
| `apps/web/src/components/SessionCard.tsx` | Archive badge |
| `apps/web/src/components/CompactSessionTable.tsx` | Archive badge |

## What We're NOT Building

- No config UI for backup path (auto-detect only)
- No "import" button (automatic)
- No separate archive tab/view
- No changes to claude-backup
- No permanent decompression cache
- No opencode/codex adapters (trait ready, impls deferred)
