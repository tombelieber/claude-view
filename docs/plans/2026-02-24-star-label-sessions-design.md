# Star / Label Sessions — Feature Design

**Date:** 2026-02-24
**Status:** Approved (concept), needs implementation plan
**Priority:** Medium (after reliability release)
**Depends on:** Reliability release (#1-4 fixes)

## Problem

Users have no way to mark important sessions for later retrieval. With hundreds or thousands of sessions, finding "that one session from last week where I fixed the auth bug" requires scrolling and guessing.

## Solution

Lightweight named bookmarks on sessions. A star toggle with an optional name/label.

## Entry Points (3)

1. **Claude Code CLI** — `/star fix auth bug` mid-session to star the current session with a name
2. **Mission Control session card** — star toggle on the monitoring dashboard
3. **Every session surface in the web app** — session list, session detail, project sessions, search results. Anywhere a session is rendered, the star affordance must be present.

The star is a **universal session primitive** — not tied to one view.

## Data Model

```sql
-- New table (or column on sessions)
ALTER TABLE sessions ADD COLUMN starred_at INTEGER NULL;  -- unix timestamp, NULL = not starred
ALTER TABLE sessions ADD COLUMN star_label TEXT NULL;      -- optional name, e.g. "fix auth bug"
```

- `starred_at` enables sort by "recently starred"
- `star_label` is optional free-text (not a fixed taxonomy)
- Architecture supports adding a separate `session_labels` table later if multi-label is needed

## CLI Integration

- Implemented as a Claude Code hook that writes to a shared store
- User types `/star fix auth bug` in a Claude Code session
- Hook writes `{ session_id, label, timestamp }` to a file/DB readable by claude-view
- claude-view picks it up on next index cycle or via file watcher

## UI Design

### Star Icon
- Subtle outline star on each session row (unstarred)
- Filled gold star when starred
- Single click toggles star on/off
- Keyboard shortcut `s` to toggle on focused/selected session

### Star with Label
- Click star → star toggles on immediately (zero friction)
- Optional: small inline text input appears to add a name (auto-dismiss after 3s if ignored)
- Or: right-click / long-press star → popover to add/edit label
- Label shown as subtle text next to session preview

### Filtering
- "Starred" filter chip in the filter bar (all session list views)
- Composable with existing filters (project + time + starred)
- Sort option: "Recently starred"

### Mission Control
- Star icon on session cards
- Same toggle behavior

## UX Principles Applied

- **Every prompt = 1 decision = friction:** Star is binary toggle (1 click). Label is optional (0 friction if you skip it).
- **Universal primitive:** Shows everywhere a session appears, not just one view.
- **Progressive disclosure:** Star first (instant), label second (optional).

## Competitive Context

- No competitor in the Claude Code viewer space has session starring/labeling
- Pattern proven by: Gmail (star), GitHub (star repos + lists), Slack (star messages)
- Gmail started with just star, added labels — same progressive approach

## Open Questions

- [ ] Where does the CLI hook write to? (File in `~/.claude/`? SQLite? Needs to work with sandbox constraints from #1)
- [ ] Should starred sessions be included in backup (#8)?
- [ ] Should star data sync across machines? (Deferred — local-first for now)
