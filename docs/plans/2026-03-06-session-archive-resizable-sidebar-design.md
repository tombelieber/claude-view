# Session Archive & Resizable Sidebar Design

**Date:** 2026-03-06
**Status:** Approved

## Feature 1: Session Archive

### Problem
Users cannot remove sessions from history. Old, irrelevant, or test sessions clutter the list with no way to clean up.

### Solution
Archive sessions by moving JSONL files to `~/.claude-view/archives/` and flagging them in SQLite. Fully reversible.

### Data Flow

```
UI action → POST /api/sessions/{id}/archive
  → mv ~/.claude/projects/{dir}/{id}.jsonl → ~/.claude-view/archives/{dir}/{id}.jsonl
  → UPDATE sessions SET archived_at = NOW() WHERE id = ?
  → 200 OK

UI action → POST /api/sessions/{id}/unarchive
  → mv file back to original location
  → UPDATE sessions SET archived_at = NULL WHERE id = ?
  → 200 OK

Bulk: POST /api/sessions/archive (body: { ids: [...] })
  → same logic per session, transactional
```

### DB Change
Add `archived_at TEXT NULL` column to `sessions` table. All existing list queries add `WHERE archived_at IS NULL` unless "show archived" filter is active.

### Indexer Guard
On re-index, check for archive records before adding sessions. Prevents re-surfacing archived sessions.

### Archive Directory Structure
```
~/.claude-view/archives/
  {encoded_project_dir}/
    {session_id}.jsonl
```
Mirrors original Claude Code directory structure for easy move-back.

### UI Surfaces

**Hover action:** Archive icon appears on hover over any session card/row in HistoryView.

**Right-click context menu:** Radix `ContextMenu` on session cards with "Archive session" option.

**Bulk mode:** Checkbox on each card. Top toolbar: "N selected — Archive | Cancel". Confirmation dialog shows count.

**Confirmation:** Dialog for single archive. Batch confirms count.

**Undo toast:** After archiving, toast with "Undo" button (5s window, calls unarchive endpoint).

**View archived:** Filter toggle in HistoryView toolbar — "Show archived" reveals archived sessions with muted style and "Unarchive" action.

### API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/sessions/{id}/archive` | Archive single session |
| POST | `/api/sessions/{id}/unarchive` | Unarchive single session |
| POST | `/api/sessions/archive` | Bulk archive (body: `{ ids: [...] }`) |
| POST | `/api/sessions/unarchive` | Bulk unarchive (body: `{ ids: [...] }`) |

---

## Feature 2: VS Code-style Resizable/Collapsible Sidebar Sections

### Problem
The sidebar has three sections (Tabs, Scope, Recent) with fixed proportions. Users cannot adjust section heights or collapse sections they don't need.

### Solution
Use `react-resizable-panels` library for draggable dividers and collapsible sections, with layout persisted to localStorage.

### Library
`react-resizable-panels` by bvaughn — mature, battle-tested (used by Vercel, Codesandbox), handles keyboard accessibility, touch, RTL.

### Layout Structure

```tsx
<PanelGroup direction="vertical" autoSaveId="sidebar-panels">
  <Panel id="tabs" collapsible minSize={5} defaultSize={20}>
    <SectionHeader title="Navigation" collapsed={false} onToggle={...} />
    {/* existing tab links */}
  </Panel>
  <PanelResizeHandle className="sidebar-divider" />
  <Panel id="scope" collapsible minSize={10} defaultSize={55}>
    <SectionHeader title="Scope" collapsed={false} onToggle={...} />
    {/* existing project tree + branch list */}
  </Panel>
  <PanelResizeHandle className="sidebar-divider" />
  <Panel id="recent" collapsible minSize={5} defaultSize={25}>
    <SectionHeader title="Recent" collapsed={false} onToggle={...} />
    {/* existing QuickJumpZone */}
  </Panel>
</PanelGroup>
```

### Section Headers
Each section gets a clickable header bar:
- Section name (bold, small text)
- Chevron: ▸ when collapsed, ▾ when expanded
- Click header = toggle collapse
- Double-click divider = toggle collapse of section below

### Divider Styling
- Height: 4px
- Color: `bg-zinc-200 dark:bg-zinc-700`
- Hover: `hover:bg-blue-400 dark:hover:bg-blue-500`
- Cursor: `row-resize`

### Persistence
- `autoSaveId="sidebar-panels"` — library auto-saves panel sizes to localStorage
- Collapsed states stored in `useAppStore` via `onCollapse`/`onExpand` callbacks

### Collapsed State
- Only header bar visible (~28px height)
- Content hidden
- Chevron rotated to ▸
- Panel `collapsedSize` set to match header height

### Min Sizes
- Tabs: 5% (~30px) — prevents invisible without explicit collapse
- Scope: 10% (~60px)
- Recent: 5% (~30px)
