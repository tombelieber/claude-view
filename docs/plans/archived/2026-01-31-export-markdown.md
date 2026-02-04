---
status: done
date: 2026-01-31
---

# Export to Markdown Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add markdown export functionality with both download and clipboard copy options, allowing users to paste conversations back into Claude Code/claude.ai for context resumption.

**Architecture:** Create a new `export-markdown.ts` module that converts `Message[]` to structured markdown with metadata headers. Add two new button handlers in `ConversationView.tsx`: one for file download and one for clipboard copy. Include a toast notification for clipboard feedback. Keyboard shortcuts deferred to a future batch design pass.

**Tech Stack:** React, TypeScript, Lucide icons (`Copy`, `Download`), browser Clipboard API, existing `Message` type from generated types.

---

## Task 1: Create Export Markdown Library

### Files

- Create: `src/lib/export-markdown.ts`

### Step 1: Create `src/lib/export-markdown.ts`

```typescript
import type { Message, ToolCall } from '../hooks/use-session'

/**
 * Formats a timestamp for markdown display
 */
function formatTimestamp(timestamp?: string | null): string {
  if (!timestamp) return ''
  const date = new Date(timestamp)
  return date.toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })
}

/**
 * Renders tool calls as a markdown list.
 */
function renderToolCalls(toolCalls?: ToolCall[] | null): string {
  if (!toolCalls || toolCalls.length === 0) return ''
  const items = toolCalls.map((tc) => `- **${tc.name}** (x${tc.count})`).join('\n')
  return `\n\n**Tools Used:**\n${items}`
}

/**
 * Generates markdown from conversation messages with metadata header.
 * Format is optimized for pasting into Claude Code / claude.ai to resume context.
 */
export function generateMarkdown(
  messages: Message[],
  projectName?: string,
  sessionId?: string,
): string {
  const exportDate = formatTimestamp(new Date().toISOString())
  const userCount = messages.filter((m) => m.role === 'user').length
  const assistantCount = messages.filter((m) => m.role === 'assistant').length

  // Metadata header
  let md = '# Conversation Export\n\n'
  if (projectName) md += `**Project:** ${projectName}  \n`
  if (sessionId) md += `**Session:** ${sessionId}  \n`
  md += `**Exported:** ${exportDate}  \n`
  md += `**Messages:** ${messages.length} (${userCount} user, ${assistantCount} assistant)\n\n`
  md += '---\n\n'

  // Group messages into turns: a turn is a user message followed by
  // zero or more assistant messages. If the conversation starts with
  // an assistant message (rare), it gets its own turn.
  let turnNumber = 0
  for (let i = 0; i < messages.length; i++) {
    const message = messages[i]
    const isUser = message.role === 'user'

    // Start a new turn on every user message, or on the first message if it's assistant
    if (isUser || i === 0) {
      turnNumber++
      md += `## Turn ${turnNumber}\n\n`
    }

    const roleLabel = isUser ? 'User' : 'Assistant'
    const ts = formatTimestamp(message.timestamp)
    md += `**${roleLabel}:**${ts ? ` *${ts}*` : ''}\n\n`
    md += message.content
    if (message.toolCalls && message.toolCalls.length > 0) {
      md += renderToolCalls(message.toolCalls)
    }
    if (message.thinking) {
      md += `\n\n<details>\n<summary>Thinking</summary>\n\n${thinking}\n\n</details>`
    }
    md += '\n\n'
  }

  return md
}

/**
 * Copies text to clipboard. Returns true on success, false on failure.
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    return true
  } catch {
    return false
  }
}

/**
 * Triggers a file download of the markdown content.
 * Pattern matches existing `downloadHtml` in export-html.ts.
 */
export function downloadMarkdown(markdown: string, filename: string): void {
  const blob = new Blob([markdown], { type: 'text/markdown;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  URL.revokeObjectURL(url)
}
```

### Step 2: Verify TypeScript compiles

Run: `npx tsc --noEmit`

Expected: No errors.

### Step 3: Commit

```bash
git add src/lib/export-markdown.ts
git commit -m "feat: add markdown export library for clipboard and download"
```

---

## Task 2: Add Toast Notification Utility

### Files

- Create: `src/lib/toast.ts`

### Step 1: Create `src/lib/toast.ts`

The existing codebase has no toast system. We create a minimal one using only inline styles (no `<style>` element injection) to avoid DOM leaks on repeated calls.

```typescript
/**
 * Shows a temporary toast notification at the bottom-right of the screen.
 * Uses only inline styles — no <style> injection, no DOM leaks.
 */
export function showToast(message: string, duration = 2000): void {
  const toast = document.createElement('div')
  toast.textContent = message
  toast.style.cssText = [
    'position: fixed',
    'bottom: 20px',
    'right: 20px',
    'background-color: #059669',
    'color: white',
    'padding: 12px 16px',
    'border-radius: 6px',
    'font-size: 14px',
    'font-family: -apple-system, BlinkMacSystemFont, sans-serif',
    'font-weight: 500',
    'box-shadow: 0 4px 6px rgba(0,0,0,0.1)',
    'z-index: 9999',
    'opacity: 0',
    'transform: translateY(10px)',
    'transition: opacity 0.2s ease-out, transform 0.2s ease-out',
  ].join(';')

  document.body.appendChild(toast)

  // Trigger enter animation on next frame
  requestAnimationFrame(() => {
    toast.style.opacity = '1'
    toast.style.transform = 'translateY(0)'
  })

  setTimeout(() => {
    toast.style.opacity = '0'
    toast.style.transform = 'translateY(10px)'
    // Remove from DOM after fade-out completes
    const onEnd = () => {
      toast.removeEventListener('transitionend', onEnd)
      toast.remove()
    }
    toast.addEventListener('transitionend', onEnd)
    // Fallback removal if transitionend never fires
    setTimeout(() => toast.remove(), 500)
  }, duration)
}
```

### Step 2: Verify TypeScript compiles

Run: `npx tsc --noEmit`

Expected: No errors.

### Step 3: Commit

```bash
git add src/lib/toast.ts
git commit -m "feat: add toast notification utility"
```

---

## Task 3: Wire Up Handlers and Buttons in ConversationView

### Files

- Modify: `src/components/ConversationView.tsx`

### Step 1: Update imports (line 2)

Change:

```typescript
import { ArrowLeft, Download, MessageSquare } from 'lucide-react'
```

To:

```typescript
import { ArrowLeft, Download, MessageSquare, Copy } from 'lucide-react'
```

Add after the `export-html` import (line 12):

```typescript
import { generateMarkdown, downloadMarkdown, copyToClipboard } from '../lib/export-markdown'
import { showToast } from '../lib/toast'
```

### Step 2: Add handler functions (after `handleExportPdf`, line 44)

```typescript
  const handleExportMarkdown = useCallback(() => {
    if (!session) return
    const markdown = generateMarkdown(session.messages, projectName, sessionId)
    downloadMarkdown(markdown, `conversation-${sessionId}.md`)
  }, [session, projectName, sessionId])

  const handleCopyMarkdown = useCallback(async () => {
    if (!session) return
    const markdown = generateMarkdown(session.messages, projectName, sessionId)
    const ok = await copyToClipboard(markdown)
    showToast(ok ? 'Markdown copied to clipboard' : 'Failed to copy — check browser permissions', ok ? 2000 : 3000)
  }, [session, projectName, sessionId])
```

### Step 3: Add buttons to header (after the PDF button, line 142)

Must match the **exact** existing button style. Copy from the HTML/PDF buttons:

```typescript
          <button
            onClick={handleExportMarkdown}
            aria-label="Export as Markdown"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>MD</span>
            <Download className="w-4 h-4" />
          </button>
          <button
            onClick={handleCopyMarkdown}
            aria-label="Copy conversation as Markdown"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>Copy</span>
            <Copy className="w-4 h-4" />
          </button>
```

Key details matching the existing codebase:
- Same `className` as HTML/PDF buttons (from `ConversationView.tsx:130`)
- `aria-label` for accessibility (matches existing pattern)
- `<span>` text + icon layout (matches `<span>HTML</span>` pattern)
- `Copy` icon imported from `lucide-react` (already used in `CodeBlock.tsx`, `Message.tsx`)

### Step 4: Verify TypeScript compiles

Run: `npx tsc --noEmit`

Expected: No errors.

### Step 5: Commit

```bash
git add src/components/ConversationView.tsx
git commit -m "feat: add markdown export and clipboard copy to conversation view"
```

---

## Task 4: Manual QA

### Files

- No changes — verification only

### Step 1: Start dev server

Run: `bun run dev`

### Step 2: Test markdown download

1. Navigate to any conversation with multiple turns
2. Click the **MD** button in the header
3. Verify file `conversation-{id}.md` downloads
4. Open in editor and verify:
   - Header has `# Conversation Export`, project name, session ID, export date
   - Turns are numbered sequentially
   - User messages start new turns; consecutive assistant messages share a turn
   - Code blocks preserved with triple backticks (passthrough from Claude's markdown)
   - Timestamps formatted as "Jan 31, 2026, 5:30 PM"

### Step 3: Test clipboard copy

1. Click the **Copy** button
2. Verify green toast appears bottom-right: "Markdown copied to clipboard"
3. Toast fades after ~2 seconds
4. Paste (`Cmd+V`) in a text editor — verify identical content to download

### Step 4: Test edge cases

- Empty conversation → header shows "0 (0 user, 0 assistant)", no turns
- Message without timestamp → no italic timestamp after role label
- Message with thinking → renders inside `<details>` block
- Message with tool calls → "Tools Used:" list appears
- Rapid copy clicks → multiple toasts stack correctly (each has its own DOM element)

### Step 5: Test context resumption

1. Copy markdown to clipboard
2. Open Claude Code or claude.ai
3. Paste content as a message
4. Verify the LLM can parse and reference the conversation history

---

## Validation Checklist

- [ ] `src/lib/export-markdown.ts` created with `generateMarkdown`, `copyToClipboard`, `downloadMarkdown`
- [ ] `src/lib/toast.ts` created with `showToast` (inline styles only, no `<style>` leaks)
- [ ] `ConversationView.tsx` imports `Copy` from lucide, new lib functions
- [ ] Two new handlers: `handleExportMarkdown`, `handleCopyMarkdown`
- [ ] No keyboard shortcuts added (deferred to future batch design)
- [ ] Buttons match exact styling of existing HTML/PDF buttons (`aria-label`, className, icon layout)
- [ ] Turn numbering groups user+assistant messages (not `Math.floor(index/2)`)
- [ ] `toolCalls` and `thinking` accessed directly (matching `MessageTyped.tsx` and `export-html.ts` patterns)
- [ ] No dead code (`detectCodeLanguage`, `contentToMarkdown` removed)
- [ ] Toast uses only inline styles + `requestAnimationFrame` for animation
- [ ] Toast cleanup uses `transitionend` listener + fallback `setTimeout`
- [ ] Build passes: `npx tsc --noEmit` and `bun run build`
