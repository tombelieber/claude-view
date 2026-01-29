---
status: done
date: 2026-01-28
---

# XML Card Full Coverage Design

## Problem

The session viewer suppresses or ignores several semantic XML tag types that appear in Claude Code session JSONL files. Slash command invocations (`/commit`, `/review-pr`, etc.) are completely invisible. Tool errors and sandboxed external content have no dedicated rendering.

## Current State

### Has dedicated UI (5 types)

| Tag | UI Treatment |
|-----|-------------|
| `<observed_from_primary_session>` | Collapsible Tool Call card |
| `<observation>` | Collapsible Observation card |
| `<tool_call>` | Generic collapsible tool card |
| `<local-command-stdout/stderr>` | Terminal-style inline block |
| `<task-notification>` | Agent status card with status icon |

### Correctly hidden (system noise)

| Tag | Reason |
|-----|--------|
| `<system-reminder>` | Internal system prompts |
| `<local-command-caveat>` | System metadata |
| `<claude-mem-context>` | Memory plugin context |
| `<command-args>` (empty) | No content to show |

### Missing UI (this design)

| Tag | What it is | Current behavior |
|-----|-----------|-----------------|
| `<command-name>` | Slash command (e.g. `/commit`) | Hidden |
| `<command-message>` | Command label | Hidden |
| `<command-args>` (non-empty) | Pasted content / arguments | Hidden |
| `<tool_use_error>` | Tool failure message | Falls to generic unknown |
| `<untrusted-data-*>` | Sandboxed external content | Falls to generic unknown |

## Design

### 1. Command Invocation Card

When a user runs a slash command, three tags appear together in the user message:

```
<command-name>/commit</command-name>
<command-message>commit</command-message>
<command-args>fix typo and update docs</command-args>
```

**Rendering:**

- Group all three tags into a single **Command Card**
- Icon: `Zap` (lightning bolt — user-initiated action)
- Header: command name in monospace, e.g. **`/commit`**
- Body (from `<command-args>` if non-empty):
  - ≤10 lines: expanded by default, rendered as markdown
  - \>10 lines: collapsed by default, first line shown as preview
- Visual: solid border with indigo/purple left accent (distinct from gray system cards — signals "user action")

**Detection:** In `extractXmlBlocks`, replace the three separate hidden patterns with a single grouped detection that emits type `'command'`.

### 2. Tool Use Error Card

`<tool_use_error>` wraps error messages when a tool invocation fails.

**Rendering:**

- Icon: `XCircle` (red)
- Header: **"Tool Error"** — extract error type from content if parseable
- Body: full error message in red-tinted terminal-style block (`bg-red-950`, monospace)
- Always expanded — errors are rare and important, never hide them
- Visual: solid border with red left accent

**Detection:** Add `<tool_use_error>` pattern to `extractXmlBlocks` with type `'tool_error'`.

### 3. Untrusted Data Card

`<untrusted-data-{uuid}>` wraps external content sandboxed by Claude for safety (fetched web content, API responses, external file reads).

**Rendering:**

- Icon: `Shield`
- Header: **"External Content"** — neutral, non-alarming
- Body: rendered as markdown inside the card
  - ≤10 lines: expanded by default
  - \>10 lines: collapsed by default
- Strip UUID from display (internal detail)
- Visual: **dashed** border with amber/yellow left accent (communicates "came from outside")

**Detection:** Add `<untrusted-data-` prefix pattern to `extractXmlBlocks` with type `'untrusted_data'`.

## Summary

| Type | Icon | Header | Default State | Border |
|------|------|--------|--------------|--------|
| Command | `Zap` | `/command-name` (mono) | Expanded ≤10 lines | Solid, indigo accent |
| Tool Error | `XCircle` | "Tool Error" | Always expanded | Solid, red accent |
| Untrusted Data | `Shield` | "External Content" | Collapsed >10 lines | Dashed, amber accent |

## Implementation

All changes are in two files:

1. **`src/components/XmlCard.tsx`** — Add three new type variants, parsers, and renderers
2. **`src/components/Message.tsx`** — No changes needed if `extractXmlBlocks` handles the new types correctly (the existing rendering pipeline already routes XML blocks to `XmlCard`)

### Changes to `XmlCard.tsx`

1. Extend `XmlCardProps['type']` union with `'command' | 'tool_error' | 'untrusted_data'`
2. Add `parseCommand()` function to extract name + args from grouped tags
3. Add rendering branches in the main `XmlCard` component for each new type
4. Update `extractXmlBlocks()`:
   - Replace three hidden `command-*` patterns with a grouped command detection
   - Add `<tool_use_error>` pattern
   - Add `<untrusted-data-` prefix pattern
5. Update `getIcon()` and `getLabel()` for new types

### Line count estimate

~80-100 lines of new code, ~10 lines of deleted hidden patterns.
