# Rich Tool Card Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Redesign the RichPane verbose mode so tool_use + tool_result render as paired cards with structured info — matching the quality of built-in tool renderers for MCP/Skill/all tool types.

**Architecture:** Extract shared helpers from RichPane.tsx, create a `usePairedMessages()` hook that merges consecutive tool_use/tool_result into display pairs, build a `SmartMcpRenderer` for auto-detecting MCP param types, and wire it all into a new `PairedToolCard` that replaces separate ToolUseMessage + ToolResultMessage components.

**Tech Stack:** React + TypeScript, Vitest (happy-dom), Tailwind CSS, existing components (CompactCodeBlock, JsonTree, Chip, FilePathHeader).

**Design doc:** `docs/plans/2026-02-22-rich-tool-card-redesign-design.md`

---

### Task 1: Extract shared helpers from RichPane into lib modules

Several functions in `RichPane.tsx` are needed by the new `PairedToolCard`. Extract them into reusable modules before creating new components.

**Files:**
- Create: `src/lib/content-detection.ts`
- Create: `src/lib/content-detection.test.ts`
- Modify: `src/components/live/RichPane.tsx` (lines 189-275, 340-367)
- Modify: `src/components/live/ToolRenderers.tsx` (lines 53-94 — export helpers)

**Step 1: Create `src/lib/content-detection.ts`**

Move these functions from `RichPane.tsx` (lines 189-275) into a new file:
- `tryParseJson(str: string): unknown | null`
- `isJsonContent(content: string): boolean`
- `isDiffContent(content: string): boolean`
- `isCodeLikeContent(content: string): boolean`
- `stripLineNumbers(content: string): string`
- `detectCodeLanguage(content: string): string`

Also move these tool name helpers from `RichPane.tsx` (lines 340-367):
- `shortenToolName(name: string): { short: string; server?: string }`
- `toolChipColor(name: string): string`

```typescript
// src/lib/content-detection.ts

/** Try to parse a string as JSON. Returns parsed value or null. */
export function tryParseJson(str: string): unknown | null {
  try {
    const trimmed = str.trim()
    if ((!trimmed.startsWith('{') && !trimmed.startsWith('[')) || trimmed.length < 2) return null
    return JSON.parse(trimmed)
  } catch {
    return null
  }
}

export function isJsonContent(content: string): boolean {
  return tryParseJson(content) !== null
}

export function isDiffContent(content: string): boolean {
  if (content.startsWith('diff --git') || content.startsWith('---') || content.startsWith('Index:')) return true
  const lines = content.split('\n')
  if (lines.length < 3) return false
  const nonEmpty = lines.filter((l) => l.length > 0)
  if (nonEmpty.length < 3) return false
  const diffLines = nonEmpty.filter((l) => /^[+-][^+-]/.test(l) || l.startsWith('@@')).length
  return diffLines / nonEmpty.length >= 0.3
}

const LINE_NUM_RE = /^\s*\d+[→\t|:]/
export function isCodeLikeContent(content: string): boolean {
  const lines = content.split('\n')
  if (lines.length < 2) return false
  const nonEmpty = lines.filter((l) => l.trim().length > 0)
  if (nonEmpty.length < 2) return false
  const matching = nonEmpty.filter((l) => LINE_NUM_RE.test(l)).length
  return matching / nonEmpty.length >= 0.4
}

export function stripLineNumbers(content: string): string {
  return content.replace(/^\s*\d+[→\t|]\s?/gm, '')
}

// Copy the full detectCodeLanguage function from RichPane.tsx lines 241-275 verbatim
export function detectCodeLanguage(content: string): string { /* ... */ }

/** Shorten verbose MCP tool names: "mcp__chrome-devtools__take_snapshot" -> "take_snapshot" */
export function shortenToolName(name: string): { short: string; server?: string } {
  const mcpMatch = /^mcp__([^_]+(?:_[^_]+)*)__(.+)$/.exec(name)
  if (mcpMatch) {
    return { short: mcpMatch[2], server: mcpMatch[1] }
  }
  return { short: name }
}

/** Pick a distinct color class based on tool name. */
export function toolChipColor(name: string): string {
  if (name.startsWith('mcp__')) return 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300'
  if (name === 'Task') return 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300'
  if (name === 'Skill') return 'bg-purple-500/10 dark:bg-purple-500/20 text-purple-700 dark:text-purple-300'
  if (name === 'Read' || name === 'Glob' || name === 'Grep') return 'bg-emerald-500/10 dark:bg-emerald-500/20 text-emerald-700 dark:text-emerald-300'
  if (name === 'Write' || name === 'Edit') return 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300'
  if (name === 'Bash') return 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300'
  return 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300'
}
```

**Step 2: Write tests for extracted helpers**

```typescript
// src/lib/content-detection.test.ts
import { describe, it, expect } from 'vitest'
import {
  tryParseJson, isJsonContent, isDiffContent, isCodeLikeContent,
  stripLineNumbers, detectCodeLanguage, shortenToolName, toolChipColor,
} from './content-detection'

describe('tryParseJson', () => {
  it('parses valid JSON object', () => {
    expect(tryParseJson('{"a":1}')).toEqual({ a: 1 })
  })
  it('returns null for plain text', () => {
    expect(tryParseJson('hello world')).toBeNull()
  })
  it('returns null for empty string', () => {
    expect(tryParseJson('')).toBeNull()
  })
})

describe('isDiffContent', () => {
  it('detects diff --git header', () => {
    expect(isDiffContent('diff --git a/file.ts b/file.ts')).toBe(true)
  })
  it('rejects normal text', () => {
    expect(isDiffContent('This is just text')).toBe(false)
  })
})

describe('shortenToolName', () => {
  it('extracts tool name and server from MCP tool', () => {
    expect(shortenToolName('mcp__chrome-devtools__take_snapshot'))
      .toEqual({ short: 'take_snapshot', server: 'chrome-devtools' })
  })
  it('returns name as-is for non-MCP tool', () => {
    expect(shortenToolName('Edit')).toEqual({ short: 'Edit' })
  })
})

describe('toolChipColor', () => {
  it('returns blue for MCP tools', () => {
    expect(toolChipColor('mcp__chrome__click')).toContain('blue')
  })
  it('returns purple for Skill', () => {
    expect(toolChipColor('Skill')).toContain('purple')
  })
})
```

**Step 3: Run tests**

Run: `bunx vitest run src/lib/content-detection.test.ts`
Expected: All pass

**Step 4: Export shared components from ToolRenderers.tsx**

Add `export` to `Chip`, `FilePathHeader`, `InlineMarkdown` in `src/components/live/ToolRenderers.tsx` (currently private functions at lines 53, 69, 88). Change:
- `function Chip(` -> `export function Chip(`
- `function FilePathHeader(` -> `export function FilePathHeader(`
- `function InlineMarkdown(` -> `export function InlineMarkdown(`

**Step 5: Update RichPane.tsx imports**

Replace the local function definitions in RichPane.tsx with imports from `content-detection.ts`:

```typescript
import {
  tryParseJson, isJsonContent, isDiffContent, isCodeLikeContent,
  stripLineNumbers, detectCodeLanguage, shortenToolName, toolChipColor,
} from '../../lib/content-detection'
```

Delete lines 189-275 (content detection functions) and lines 340-367 (tool name helpers) from RichPane.tsx.

**Step 6: Run existing tests to confirm no regressions**

Run: `bunx vitest run`
Expected: All existing tests still pass

**Step 7: Commit**

```
refactor: extract content-detection helpers and export ToolRenderer shared components
```

---

### Task 2: Create `usePairedMessages` hook

Pure logic hook that pairs `tool_use` + `tool_result` messages into display items.

**Files:**
- Create: `src/hooks/use-paired-messages.ts`
- Create: `src/hooks/use-paired-messages.test.ts`

**Step 1: Write failing tests**

```typescript
// src/hooks/use-paired-messages.test.ts
import { describe, it, expect } from 'vitest'
import { pairMessages } from './use-paired-messages'
import type { RichMessage } from '../components/live/RichPane'

function msg(type: RichMessage['type'], overrides: Partial<RichMessage> = {}): RichMessage {
  return { type, content: '', ts: Date.now() / 1000, ...overrides }
}

describe('pairMessages', () => {
  it('wraps non-tool messages as kind=message', () => {
    const input = [msg('user'), msg('assistant')]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('message')
    expect(result[1].kind).toBe('message')
  })

  it('pairs tool_use with its subsequent tool_result', () => {
    const input = [
      msg('tool_use', { name: 'Edit', ts: 100 }),
      msg('tool_result', { content: 'ok', ts: 101 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(1)
    expect(result[0].kind).toBe('tool_pair')
    if (result[0].kind === 'tool_pair') {
      expect(result[0].toolUse.name).toBe('Edit')
      expect(result[0].toolResult?.content).toBe('ok')
    }
  })

  it('skips thinking messages between tool_use and tool_result', () => {
    const input = [
      msg('tool_use', { name: 'Bash', ts: 100 }),
      msg('thinking', { content: 'hmm', ts: 100.5 }),
      msg('tool_result', { content: 'done', ts: 101 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('message') // thinking
    expect(result[1].kind).toBe('tool_pair')
  })

  it('emits unpaired tool_use with null toolResult when no result follows', () => {
    const input = [
      msg('tool_use', { name: 'Read', ts: 100 }),
      msg('user', { content: 'hello', ts: 102 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('tool_pair')
    if (result[0].kind === 'tool_pair') {
      expect(result[0].toolResult).toBeNull()
    }
    expect(result[1].kind).toBe('message')
  })

  it('handles consecutive tool pairs', () => {
    const input = [
      msg('tool_use', { name: 'Read', ts: 100 }),
      msg('tool_result', { content: 'file content', ts: 101 }),
      msg('tool_use', { name: 'Edit', ts: 102 }),
      msg('tool_result', { content: 'ok', ts: 103 }),
    ]
    const result = pairMessages(input)
    expect(result).toHaveLength(2)
    expect(result[0].kind).toBe('tool_pair')
    expect(result[1].kind).toBe('tool_pair')
  })

  it('handles empty input', () => {
    expect(pairMessages([])).toEqual([])
  })

  it('handles tool_result without preceding tool_use as standalone', () => {
    const input = [msg('tool_result', { content: 'orphan' })]
    const result = pairMessages(input)
    expect(result).toHaveLength(1)
    expect(result[0].kind).toBe('message')
  })
})
```

**Step 2: Run tests to verify they fail**

Run: `bunx vitest run src/hooks/use-paired-messages.test.ts`
Expected: FAIL — module not found

**Step 3: Implement the hook**

```typescript
// src/hooks/use-paired-messages.ts
import { useMemo } from 'react'
import type { RichMessage } from '../components/live/RichPane'

export type DisplayItem =
  | { kind: 'message'; message: RichMessage }
  | { kind: 'tool_pair'; toolUse: RichMessage; toolResult: RichMessage | null }

/**
 * Pure pairing function (exported for testing without React).
 * Walks RichMessage[] and pairs each tool_use with its subsequent tool_result.
 * Thinking messages between a tool_use and its result are emitted as standalone
 * messages before the pair.
 */
export function pairMessages(messages: RichMessage[]): DisplayItem[] {
  const items: DisplayItem[] = []
  let i = 0

  while (i < messages.length) {
    const m = messages[i]

    if (m.type === 'tool_use') {
      const skipped: RichMessage[] = []
      let resultMsg: RichMessage | null = null
      let j = i + 1

      while (j < messages.length) {
        const next = messages[j]
        if (next.type === 'tool_result') {
          resultMsg = next
          break
        }
        if (next.type === 'thinking') {
          skipped.push(next)
          j++
          continue
        }
        break
      }

      for (const s of skipped) {
        items.push({ kind: 'message', message: s })
      }

      items.push({ kind: 'tool_pair', toolUse: m, toolResult: resultMsg })
      i = resultMsg ? j + 1 : (skipped.length > 0 ? j : i + 1)
      continue
    }

    items.push({ kind: 'message', message: m })
    i++
  }

  return items
}

/**
 * React hook that memoizes message pairing.
 */
export function usePairedMessages(messages: RichMessage[]): DisplayItem[] {
  return useMemo(() => pairMessages(messages), [messages])
}
```

**Step 4: Run tests to verify they pass**

Run: `bunx vitest run src/hooks/use-paired-messages.test.ts`
Expected: All pass

**Step 5: Commit**

```
feat: add usePairedMessages hook for tool_use/tool_result pairing
```

---

### Task 3: Add SmartMcpRenderer to ToolRenderers

A generic renderer for MCP tools that auto-detects parameter types and renders each with the appropriate component.

**Files:**
- Modify: `src/components/live/ToolRenderers.tsx`

**Step 1: Add SmartMcpRenderer**

Add after the existing renderers (before the Registry section, around line 467). The SmartMcpRenderer auto-detects param types: URLs get Link icon + clickable href, file paths get FilePathHeader, selector keys (uid, selector, xpath, css) get amber highlighted chips, multi-line strings get CompactCodeBlock, booleans get colored chips, numbers get Chip, objects/arrays get CompactCodeBlock with JSON, short strings get Chip.

**Step 2: Update the registry to support MCP fallback**

Replace the existing `toolRendererRegistry` export with a registry + lookup function:

```typescript
const builtinRendererRegistry: Record<string, React.ComponentType<ToolRendererProps>> = {
  // ... existing entries unchanged ...
}

/** Look up a tool renderer. Falls back to SmartMcpRenderer for mcp__* tools. */
export function getToolRenderer(name: string): React.ComponentType<ToolRendererProps> | null {
  if (builtinRendererRegistry[name]) return builtinRendererRegistry[name]
  if (name.startsWith('mcp__')) return SmartMcpRenderer
  return null
}

// Keep backward-compat export
export const toolRendererRegistry = builtinRendererRegistry
```

**Step 3: Run tests**

Run: `bunx vitest run`
Expected: All pass

**Step 4: Commit**

```
feat: add SmartMcpRenderer with auto-detection for MCP tool params
```

---

### Task 4: Enhance SkillRenderer and TaskRenderer

Make Skill and Task tool renderers show more structured info.

**Files:**
- Modify: `src/components/live/ToolRenderers.tsx` (lines 260-298)

**Step 1: Enhance SkillRenderer**

Replace the plain text `{args}` div with `<InlineMarkdown text={args} />` so args render as rich markdown instead of raw text.

**Step 2: Enhance TaskRenderer**

Add `model` and `mode` chip fields. Change `name` from Chip to bold label. Change `description` from Chip to subtitle text below the header. Keep `prompt` as InlineMarkdown.

**Step 3: Run tests**

Run: `bunx vitest run`
Expected: All pass

**Step 4: Commit**

```
feat: enhance SkillRenderer and TaskRenderer with richer info display
```

---

### Task 5: Create PairedToolCard component

The unified card that renders tool_use input + tool_result output together.

**Files:**
- Create: `src/components/live/PairedToolCard.tsx`

**Step 1: Create the component**

PairedToolCard takes `{ toolUse, toolResult, index, verboseMode }` props. Layout:
- Header row: Wrench icon + name chip (colored by category) + server name + { } toggle + timestamp
- Input section: Always visible. Uses `getToolRenderer()` for rich mode, CompactCodeBlock for JSON mode
- Result section: Separator line + status icon (CheckCircle/XCircle) + "result"/"error" label + duration badge + content (auto-detected: JSON -> JsonTree, diff -> CompactCodeBlock, code -> CompactCodeBlock, text -> Markdown)
- If toolResult is null, shows Loader2 spinner + "pending..."
- Special case: AskUserQuestion still gets its dedicated card in non-verbose mode

Import content detection functions from `src/lib/content-detection.ts`. Import `getToolRenderer` from ToolRenderers. Import `markdownComponents` from RichPane (or later from shared module).

**Step 2: Verify compilation**

Run: `bunx vitest run`
Expected: All pass

**Step 3: Commit**

```
feat: add PairedToolCard component for unified tool input+output display
```

---

### Task 6: Wire PairedToolCard into RichPane

Replace the old separate ToolUseMessage + ToolResultMessage dispatching with the new pairing system.

**Files:**
- Modify: `src/components/live/RichPane.tsx`

**Step 1: Add imports**

```typescript
import { usePairedMessages, type DisplayItem } from '../../hooks/use-paired-messages'
import { PairedToolCard } from './PairedToolCard'
```

**Step 2: Create DisplayItemCard dispatch**

New function that dispatches on `DisplayItem.kind`:
- `tool_pair` -> `PairedToolCard`
- `message` with `tool_use` type -> `PairedToolCard` with null result (orphan)
- `message` with `tool_result` type -> existing `ToolResultMessage` (orphan)
- All other message types -> existing renderers unchanged

**Step 3: Apply `usePairedMessages` after `displayMessages`**

```typescript
const displayItems = usePairedMessages(displayMessages)
```

**Step 4: Update Virtuoso**

Change `data={displayMessages}` to `data={displayItems}` and `itemContent` to use `DisplayItemCard`.

**Step 5: Update all `displayMessages.length` refs to `displayItems.length`**

Several refs in scroll logic, prev count tracking, empty state check.

**Step 6: Remove unused `ToolUseMessage` component**

Delete the function (lines 425-516). Keep `ToolResultMessage` for orphan results. Remove `JsonKeyValueChips` import if no longer used.

**Step 7: Run tests**

Run: `bunx vitest run`
Expected: All pass

**Step 8: Manual verification**

Run: `bun run dev`

Verify in browser:
- Built-in tool calls show paired input+output cards
- MCP tool calls show smart-detected params
- Skill tool calls show InlineMarkdown args
- Task tool calls show subagent_type, name, description, prompt
- Duration badge shows between input and output
- { } toggle switches between rich and JSON view
- Pending tools show "pending..." indicator
- Filter chips still work in verbose mode
- Scroll-to-bottom and "New messages" pill still work

**Step 9: Commit**

```
feat: wire PairedToolCard into RichPane, replacing separate tool_use/tool_result rendering
```

---

### Task 7: Extract shared markdownComponents (cleanup)

The `markdownComponents` object is duplicated between `RichPane.tsx` and `PairedToolCard.tsx`. Extract to a shared module.

**Files:**
- Create: `src/lib/markdown-components.tsx`
- Modify: `src/components/live/RichPane.tsx`
- Modify: `src/components/live/PairedToolCard.tsx`

**Step 1: Create shared module**

Move the `markdownComponents` constant and `mdBlockCounter` into `src/lib/markdown-components.tsx`. Import CompactCodeBlock from the components directory.

**Step 2: Replace local copies with import in both files**

```typescript
import { markdownComponents } from '../../lib/markdown-components'
```

**Step 3: Run tests**

Run: `bunx vitest run`
Expected: All pass

**Step 4: Commit**

```
refactor: extract shared markdownComponents to lib module
```

---

## Summary

| Task | Files | Description |
|------|-------|-------------|
| 1 | `content-detection.ts`, ToolRenderers, RichPane | Extract shared helpers |
| 2 | `use-paired-messages.ts` + test | Pairing hook with tests |
| 3 | ToolRenderers.tsx | SmartMcpRenderer + getToolRenderer |
| 4 | ToolRenderers.tsx | Enhanced Skill/Task renderers |
| 5 | `PairedToolCard.tsx` | Unified tool card component |
| 6 | RichPane.tsx | Wire everything together |
| 7 | `markdown-components.tsx` | DRY cleanup |

**New files:** `content-detection.ts`, `content-detection.test.ts`, `use-paired-messages.ts`, `use-paired-messages.test.ts`, `PairedToolCard.tsx`, `markdown-components.tsx`

**Modified files:** `RichPane.tsx`, `ToolRenderers.tsx`

**Testing strategy:** Unit tests for pure logic (content detection, message pairing). Component behavior verified via manual browser testing in verbose mode.
