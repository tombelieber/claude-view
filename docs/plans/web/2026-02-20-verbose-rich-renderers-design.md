# Verbose Mode: Rich Tool Renderers

**Date:** 2026-02-20
**Status:** Draft

## Problem

In verbose mode, tool_use inputs are always rendered as raw JSON (`JsonTree` or `JsonKeyValueChips`). But the JSON values contain semantically rich content — code snippets, file paths, shell commands, markdown prose, diffs — that could be visualized far better. JSON should be the **fallback**, not the default.

## Design

### Rendering Hierarchy

```
tool_use arrives
  ├─ toolRendererRegistry[toolName] exists?
  │   ├─ YES → <SpecificRenderer inputData={...} />
  │   └─ NO  → current behavior (JsonKeyValueChips / JsonTree)
  └─ tool chip + timestamp (always shown)
```

### View Toggle (Rich ↔ JSON)

Two levels of control:

1. **Global toggle** in the terminal overlay header (next to existing `verbose` button):
   - Button labeled `rich` / `json` (only visible when verbose mode is on)
   - Sets the default rendering mode for ALL tool cards
   - Persisted in `monitor-store.ts` via `richRenderMode: 'rich' | 'json'`

2. **Per-card override** — small `{ }` icon button on each tool_use card:
   - Click flips that individual card between rich and JSON
   - Local state only (not persisted, resets on remount)
   - Overrides the global setting for that specific card

**Resolution logic:**
```
effectiveMode = cardOverride ?? globalDefault
```

### Store Changes

```typescript
// monitor-store.ts additions
interface MonitorState {
  // ... existing fields ...
  richRenderMode: 'rich' | 'json'   // NEW: global default for verbose tool rendering
  setRichRenderMode: (mode: 'rich' | 'json') => void  // NEW
}
```

Persisted in the existing `claude-view:monitor-grid-prefs` localStorage key.

### Tool Renderer Registry

New file: `src/components/live/ToolRenderers.tsx`

Each renderer receives:
```typescript
interface ToolRendererProps {
  inputData: Record<string, unknown>
  name: string        // raw tool name
}
```

#### File Operation Tools

| Tool | Rendering |
|------|-----------|
| **Edit** | File path header (mono, with extension badge) → inline unified diff generated from `old_string` → `new_string`, syntax-highlighted via `CompactCodeBlock` with `language="diff"`. Shows `replace_all` as a chip if true. |
| **Write** | File path header → full `content` as syntax-highlighted code block. Language auto-detected from file extension (`.rs` → rust, `.tsx` → tsx, etc.) |
| **Read** | File path displayed prominently in mono. `offset` and `limit` shown as muted metadata chips if present. `pages` shown for PDFs. |

#### Search Tools

| Tool | Rendering |
|------|-----------|
| **Grep** | `pattern` in a regex-styled mono block (distinct background). `path`, `glob`, `type`, `output_mode` as labeled chips. Context flags (`-A`, `-B`, `-C`) shown as small badges. |
| **Glob** | `pattern` prominently displayed in mono with wildcard highlighting. `path` as muted context below. |

#### Execution Tools

| Tool | Rendering |
|------|-----------|
| **Bash** | `command` in a bash-highlighted `CompactCodeBlock`. `description` as muted text above the block. `timeout` shown as chip if non-default. |

#### Agent/Orchestration Tools

| Tool | Rendering |
|------|-----------|
| **Task** | `prompt` rendered as markdown (using existing `react-markdown` pipeline). `subagent_type` as a colored badge. `description` and `name` as chips. |
| **Skill** | `skill` name as a prominent badge. `args` as muted mono text. |
| **SendMessage** | `recipient` as a badge. `content` as markdown. `type` as a chip. |
| **TaskCreate** | `subject` as bold text. `description` as markdown body. `activeForm` as muted italic. |
| **TaskUpdate** | `taskId` as badge. Changed fields shown as labeled chips (status with color coding). |
| **TaskList/TaskGet** | Minimal — just the tool chip, inputs are trivial. |

#### Web Tools

| Tool | Rendering |
|------|-----------|
| **WebFetch** | `url` as a clickable link (opens in new tab). `prompt` as markdown text below. |
| **WebSearch** | `query` in a search-box-styled container. Domain filters as chips. |

#### Notebook/Other Tools

| Tool | Rendering |
|------|-----------|
| **NotebookEdit** | Notebook path header. `cell_number`/`cell_id` as badge. `new_source` as syntax-highlighted code. `edit_mode` and `cell_type` as chips. |
| **AskUserQuestion** | Already has `AskUserQuestionDisplay` — no changes needed. |
| **EnterPlanMode / ExitPlanMode** | Minimal — just the tool chip. |

#### Fallback

Any tool NOT in the registry (including all MCP `mcp__*` tools) → current `JsonKeyValueChips` (collapsed) / `JsonTree` (expanded) behavior. Unchanged.

### Diff Generation for Edit Tool

Client-side, generate a unified diff from `old_string` → `new_string`:

```typescript
function generateInlineDiff(oldStr: string, newStr: string): string {
  const oldLines = oldStr.split('\n')
  const newLines = newStr.split('\n')
  let diff = ''
  for (const line of oldLines) diff += `- ${line}\n`
  for (const line of newLines) diff += `+ ${line}\n`
  return diff.trimEnd()
}
```

This is a simple line-level diff (not a proper LCS diff). The old/new strings are typically short (a few lines), so a naive approach is sufficient. If we want smarter diffs later, we can add `diff` as a dependency.

### File Extension → Language Map

```typescript
const EXT_LANG: Record<string, string> = {
  rs: 'rust', ts: 'typescript', tsx: 'tsx', js: 'javascript',
  jsx: 'jsx', py: 'python', go: 'go', sql: 'sql',
  css: 'css', html: 'html', json: 'json', yaml: 'yaml',
  yml: 'yaml', toml: 'toml', md: 'markdown', sh: 'bash',
  bash: 'bash', zsh: 'bash', c: 'c', cpp: 'cpp',
  java: 'java', rb: 'ruby', swift: 'swift', kt: 'kotlin',
}

function langFromPath(filePath: string): string {
  const ext = filePath.split('.').pop()?.toLowerCase() || ''
  return EXT_LANG[ext] || 'text'
}
```

### Component Changes

**`RichPane.tsx`:**
- Import `toolRendererRegistry` from `ToolRenderers.tsx`
- In `ToolUseMessage`, check registry before falling back to JSON
- Pass `richRenderMode` (from store) and per-card override state
- Add `{ }` toggle button to tool_use card header

**`TerminalOverlay.tsx`:**
- Add `rich | json` toggle button next to existing `verbose` toggle
- Only visible when `verboseMode === true`

**`monitor-store.ts`:**
- Add `richRenderMode` field + `setRichRenderMode` action
- Add to `partialize` for persistence

### Visual Treatment

Per UI/UX Pro Max guidelines:
- All interactive elements get `cursor-pointer`
- Toggle transitions: `transition-colors duration-200`
- `{ }` per-card button: 10px mono, muted by default, highlighted when in JSON mode
- File paths: `font-mono text-[11px]` with a subtle file-icon prefix
- Chips: consistent with existing `toolChipColor` palette
- No emojis — use Lucide icons only
- Dark mode: all colors use explicit Tailwind `dark:` variants (no CSS vars)

### What Does NOT Change

- Compact mode rendering — completely untouched
- `tool_result` rendering — already smart (diff/code/markdown/JSON detection)
- `assistant` / `user` message rendering — unchanged
- `thinking` message rendering — unchanged
- MCP tool rendering — falls through to existing JSON behavior
- WebSocket data format — no backend changes needed

## Files Touched

| File | Change |
|------|--------|
| `src/components/live/ToolRenderers.tsx` | **NEW** — all tool-specific renderers + registry |
| `src/components/live/RichPane.tsx` | Import registry, modify `ToolUseMessage` to check it |
| `src/components/live/TerminalOverlay.tsx` | Add `rich | json` global toggle |
| `src/store/monitor-store.ts` | Add `richRenderMode` state |

## Dependencies

No new npm dependencies. Uses existing:
- `CompactCodeBlock` for syntax highlighting
- `react-markdown` + `remarkGfm` + `rehypeRaw` for markdown
- `lucide-react` for icons
