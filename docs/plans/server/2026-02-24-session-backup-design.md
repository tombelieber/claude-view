# Session Backup — Feature Design

**Date:** 2026-02-24
**Status:** Draft (needs approach evaluation)
**Priority:** High (after reliability release)
**Depends on:** Reliability release (#1-4 fixes)

## Problem

Claude Code retains session history for only **30 days**, then purges. Uninstalling Claude Code may wipe `~/.claude/` entirely. For power users, losing chat history means losing institutional knowledge of every coding decision, debug session, and architectural choice.

> Imagine WhatsApp not able to recover chat history — then it's a useless app.

## Goal

claude-view preserves session data indefinitely, surviving both Claude Code's 30-day retention and full uninstalls.

## Approach Options (TBD — needs evaluation)

### Option A: Local Archive (claude-view never forgets)

claude-view continuously copies/archives JSONL session data into its own storage directory before Claude Code's 30-day retention deletes them.

| Pros | Cons |
|------|------|
| Zero infrastructure | Still local (disk failure = gone) |
| No privacy concerns | No cross-device access |
| Works offline | Doubles storage usage |
| Simple to implement | |

**How it would work:**
- Background job compares `~/.claude/projects/` against archive
- New/modified JSONL files are copied to claude-view's data dir
- When Claude Code deletes a session, claude-view still has it
- Sessions from archive show a "archived" badge in UI

### Option B: iCloud Sync (like Apple Photos)

Symlink or sync claude-view's data directory to iCloud Drive.

| Pros | Cons |
|------|------|
| Zero infrastructure cost | macOS-only |
| Automatic sync | iCloud flaky with many small files |
| Cross-device (Mac to Mac) | Storage limits (5GB free) |
| User already trusts iCloud | No Linux/Windows |

### Option C: Cloud Backup (like Telegram)

Encrypted upload to a cloud service (S3, Supabase Storage, R2).

| Pros | Cons |
|------|------|
| Cross-platform, cross-device | Privacy (sessions contain code) |
| Survives machine wipe | Infrastructure cost |
| Sharable across machines | Needs auth (Supabase already planned for mobile) |
| Professional backup story | More complex to implement |

### Option D: Hybrid — Local Archive + Optional Cloud

- **Default:** Local archive (Option A) — always on, zero config
- **Optional:** Cloud sync for users who want cross-device (Option C)
- Progressive: start with A, add C later

## Recommended Direction

**Option D (Hybrid)** — but start with just Option A (local archive) for the initial release. It's the highest value with lowest complexity. Cloud sync can layer on top later, potentially reusing the Supabase infrastructure from the mobile-remote feature.

## Data Considerations

- Average JSONL file size: varies (small sessions ~10KB, large agentic sessions ~5MB+)
- 4146 JSONL files on dev machine
- Storage estimate: likely 500MB-2GB for a heavy user
- Compression (gzip/zstd) could reduce by ~70%

## Open Questions

- [ ] What triggers the archive? (Background timer? File watcher? On-access?)
- [ ] How to handle the sandbox constraint (#1)? Archive dir must be writable.
- [ ] Should archived sessions be searchable via Tantivy? (Yes, probably)
- [ ] Retention policy for archives? (Infinite? User-configurable?)
- [ ] Should backup include star/label data (#5)?
- [ ] Format: raw JSONL copy, or claude-view's own format (SQLite rows)?

## Competitive Context

- No competitor in this space offers session backup
- Claude Code's 30-day retention is a known pain point in the community
- This would be a unique selling point for claude-view

## Next Steps

- [ ] Evaluate approaches in a dedicated design session
- [ ] Prototype Option A (local archive) to validate storage/performance
- [ ] Design the archive discovery and UI (archived badge, search integration)
