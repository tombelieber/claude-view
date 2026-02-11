---
status: done
date: 2026-02-12
---

# "Continue This Chat" Feature — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the 5-button export toolbar with a single primary "Continue This Chat" button (copies a condensed LLM-ready context prompt to clipboard) and an overflow menu for archive exports.

**Architecture:** New `generateResumeContext()` function in `src/lib/export-markdown.ts` that takes both `Message[]` (from `useSession`) and `SessionDetail` (from `useSessionDetail`) to produce a short, structured context prompt. The ConversationView header is restructured: primary CTA + overflow dropdown for HTML/PDF/MD/Copy.

**Tech Stack:** React, TypeScript, Lucide icons, existing `copyToClipboard()` util, existing `showToast()` util.

---

## Data Availability (verified)

| Field | Source | Available | Notes |
|-------|--------|-----------|-------|
| `summary` | `SessionDetail` | `string \| null` | Claude Code auto-summary. **May be null** for many sessions — need fallback |
| `preview` | `SessionDetail` | `string` | First user prompt, always present |
| `lastMessage` | `SessionDetail` | `string` | Last message content, always present |
| `filesRead` | `SessionDetail` | `string[]` | Full paths, deduplicated |
| `filesEdited` | `SessionDetail` | `string[]` | Full paths, deduplicated |
| `gitBranch` | `SessionDetail` | `string \| null` | May be null |
| `projectPath` | `SessionDetail` | `string` | Always present |
| `skillsUsed` | `SessionDetail` | `string[]` | May be empty |
| `durationSeconds` | `SessionDetail` | `number` | Always present |
| `primaryModel` | `SessionDetail` | `string \| null` | May be null |
| `messages` (full) | `useSession` → `ParsedSession` | `Message[]` | Full conversation, all roles |
| `categoryL1/L2/L3` | `SessionDetail` | `string \| null` | Task category if classified |

---

## Task 1: Add `generateResumeContext()` function

**Files:**
- Modify: `src/lib/export-markdown.ts`

**Step 1: Write the function**

Add to the end of `src/lib/export-markdown.ts`, before the existing `copyToClipboard`. (The `SessionDetail` import is added at the top of the file in Step 2 — do NOT paste the import here.)

```typescript
/** Truncate text at a word boundary, avoiding breaks inside code blocks. */
function truncateAtSafePoint(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text

  // If a code block starts before maxLen but doesn't close, truncate before it
  const codeBlockStart = text.indexOf('```')
  if (codeBlockStart !== -1 && codeBlockStart < maxLen) {
    const codeBlockEnd = text.indexOf('```', codeBlockStart + 3)
    if (codeBlockEnd === -1 || codeBlockEnd > maxLen) {
      const before = text.slice(0, codeBlockStart).trimEnd()
      return before.length > 0 ? before + '...' : text.slice(0, maxLen) + '...'
    }
  }

  // Break at word boundary
  const truncated = text.slice(0, maxLen)
  const lastSpace = truncated.lastIndexOf(' ')
  return (lastSpace > maxLen * 0.5 ? truncated.slice(0, lastSpace) : truncated) + '...'
}

/**
 * Generates a condensed context prompt optimized for pasting into a new
 * Claude Code session to "continue" an old conversation.
 *
 * Design constraints:
 * - Target ~200-500 tokens (not 50K like full markdown export)
 * - Structured for LLM consumption, not human reading
 * - Graceful fallbacks when fields are null
 */
export function generateResumeContext(
  messages: Message[],
  detail: SessionDetail,
): string {
  const sections: string[] = []

  // Header
  sections.push('I want to continue a previous conversation. Here is the context:\n')

  // Project context
  const projectLine = `**Project:** \`${detail.projectPath}\``
  const branchLine = detail.gitBranch ? ` (branch: \`${detail.gitBranch}\`)` : ''
  sections.push(projectLine + branchLine)

  // Summary — use Claude Code's auto-summary if present (may be null), else fall back to preview
  const summaryText = detail.summary || detail.preview || ''
  if (summaryText) {
    sections.push(`**What I was doing:** ${summaryText}`)
  }

  // Category if available
  if (detail.categoryL1) {
    const cats = [detail.categoryL1, detail.categoryL2, detail.categoryL3]
      .filter(Boolean)
      .join(' > ')
    sections.push(`**Task type:** ${cats}`)
  }

  // Files modified (deduplicated, max 15)
  const uniqueEdited = [...new Set(detail.filesEdited)]
  if (uniqueEdited.length > 0) {
    const fileList = uniqueEdited.slice(0, 15).map(f => `- \`${f}\``).join('\n')
    const suffix = uniqueEdited.length > 15
      ? `\n- ... and ${uniqueEdited.length - 15} more`
      : ''
    sections.push(`**Files modified:**\n${fileList}${suffix}`)
  }

  // Files read (top 10, excluding already-listed edited files)
  const editedSet = new Set(uniqueEdited)
  const readOnly = [...new Set(detail.filesRead)].filter(f => !editedSet.has(f))
  if (readOnly.length > 0) {
    const fileList = readOnly.slice(0, 10).map(f => `- \`${f}\``).join('\n')
    const suffix = readOnly.length > 10
      ? `\n- ... and ${readOnly.length - 10} more`
      : ''
    sections.push(`**Files referenced:**\n${fileList}${suffix}`)
  }

  // Last few conversation turns (user + assistant only, max 5 exchanges)
  const conversational = messages.filter(
    m => m.role === 'user' || m.role === 'assistant'
  )
  // Take last 10 messages (up to 5 exchanges)
  const recentMessages = conversational.slice(-10)
  if (recentMessages.length > 0) {
    const turnLines = recentMessages.map(m => {
      const role = m.role === 'user' ? 'User' : 'Assistant'
      // Truncate long messages at word boundary, avoid breaking code blocks
      const content = truncateAtSafePoint(m.content, 200)
      return `**${role}:** ${content}`
    })
    sections.push(`**Recent conversation:**\n${turnLines.join('\n\n')}`)
  }

  sections.push(
    recentMessages.length > 0
      ? '---\nPlease continue from where we left off.'
      : '---\nPlease help me with this project based on the context above.'
  )

  return sections.join('\n\n')
}
```

**Step 2: Verify the import works**

The file already imports `Message` and `ToolCall` from `'../hooks/use-session'`. We need to also import `SessionDetail` from `'../types/generated'`. Check the existing import at line 1:

```typescript
// Current:
import type { Message, ToolCall } from '../hooks/use-session'

// Change to:
import type { Message, ToolCall } from '../hooks/use-session'
import type { SessionDetail } from '../types/generated'
```

**Step 3: Commit**

```bash
git add src/lib/export-markdown.ts
git commit -m "feat: add generateResumeContext() for Continue This Chat"
```

---

## Task 2: Add `handleContinueChat` handler in ConversationView

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Add `generateResumeContext` to the existing static import**

At line 16, change:
```typescript
// Current:
import { generateMarkdown, downloadMarkdown, copyToClipboard } from '../lib/export-markdown'

// New:
import { generateMarkdown, generateResumeContext, downloadMarkdown, copyToClipboard } from '../lib/export-markdown'
```

**Step 2: Add the handler**

After the existing `handleResume` (line 120-127), add:

```typescript
  const handleContinueChat = useCallback(async () => {
    if (!session || !sessionDetail) return
    const context = generateResumeContext(session.messages, sessionDetail)
    const ok = await copyToClipboard(context)
    showToast(
      ok ? 'Context copied — paste into a new Claude session' : 'Failed to copy — check browser permissions',
      3000
    )
  }, [session, sessionDetail])
```

**Step 3: Update keyboard shortcut**

Replace the Resume shortcut (`Cmd+Shift+R`) with Continue This Chat. In the `useEffect` at line 129-149:

```typescript
  // Current (line 141-143):
  } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'r') {
    e.preventDefault()
    handleResume()
  }

  // Replace with:
  } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'r') {
    e.preventDefault()
    handleContinueChat()
  }
```

Update the deps array at line 149:
```typescript
  // Current:
  }, [handleExportHtml, handleExportPdf, handleResume])

  // New:
  }, [handleExportHtml, handleExportPdf, handleContinueChat])
```

**Step 4: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat: add handleContinueChat handler with Cmd+Shift+R shortcut"
```

---

## Task 3: Restructure the header buttons

**Files:**
- Modify: `src/components/ConversationView.tsx`
- Add import: `ChevronDown` from `lucide-react`

**Goal:** Replace the current 5-button layout:
```
[Resume] [HTML ↓] [PDF ↓] [MD ↓] [Copy]
```

With:
```
[▶ Continue This Chat]  [Export ▾]
                          ├─ Resume Command
                          ├─ HTML
                          ├─ PDF
                          ├─ Markdown
                          └─ Copy Markdown
```

**Step 1: Add `ChevronDown` to lucide imports**

```typescript
// Current (line 3):
import { ArrowLeft, Copy, Download, MessageSquare, Eye, Code, FileX, Terminal } from 'lucide-react'

// New:
import { ArrowLeft, ChevronDown, Copy, Download, MessageSquare, Eye, Code, FileX, Terminal } from 'lucide-react'
```

**Step 2: Add dropdown state**

After `const [viewMode, setViewMode] = useState<'compact' | 'full'>('compact')` (line 58), add:

```typescript
  const [exportMenuOpen, setExportMenuOpen] = useState(false)
  const exportMenuRef = useRef<HTMLDivElement>(null)
```

`useRef` is NOT currently imported. Add it to the React import at line 1:

```typescript
// Current (line 1):
import { useState, useMemo, useEffect, useCallback } from 'react'

// New:
import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
```

**Step 3: Add click-outside handler for export menu**

After the keyboard shortcut `useEffect` (after line 149), add:

```typescript
  // Close export menu on outside click
  useEffect(() => {
    if (!exportMenuOpen) return
    function handleClick(e: MouseEvent) {
      if (exportMenuRef.current && !exportMenuRef.current.contains(e.target as Node)) {
        setExportMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [exportMenuOpen])
```

**Step 4: Replace the button group (lines 362-419)**

Replace the entire `<div className="flex items-center gap-2">` block at lines 362-419 with:

```tsx
        <div className="flex items-center gap-2">
          {/* Primary CTA: Continue This Chat */}
          <button
            onClick={handleContinueChat}
            disabled={!exportsReady || !sessionDetail}
            aria-label="Copy conversation context to clipboard for continuing in a new session"
            className={cn(
              "flex items-center gap-2 px-3 py-1.5 text-sm border rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1",
              exportsReady && sessionDetail
                ? "border-blue-500 dark:border-blue-400 text-blue-700 dark:text-blue-300 bg-white dark:bg-gray-800 hover:bg-blue-50 dark:hover:bg-blue-900/30 cursor-pointer"
                : "opacity-50 cursor-not-allowed border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-400"
            )}
          >
            <Copy className="w-4 h-4" />
            <span>Continue This Chat</span>
          </button>

          {/* Export overflow menu */}
          <div className="relative" ref={exportMenuRef}>
            <button
              onClick={() => setExportMenuOpen(!exportMenuOpen)}
              disabled={!exportsReady}
              aria-label="Export options"
              aria-expanded={exportMenuOpen}
              aria-haspopup="menu"
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1.5 text-sm border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 dark:text-gray-300 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1",
                exportsReady
                  ? "hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                  : "opacity-50 cursor-not-allowed"
              )}
            >
              <Download className="w-4 h-4" />
              <span>Export</span>
              <ChevronDown className={cn("w-3.5 h-3.5 transition-transform", exportMenuOpen && "rotate-180")} aria-hidden="true" />
            </button>

            {exportMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg z-50 py-1">
                <button
                  onClick={() => { handleResume(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Terminal className="w-4 h-4" />
                  Resume Command
                </button>
                <button
                  onClick={() => { handleExportHtml(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  HTML
                </button>
                <button
                  onClick={() => { handleExportPdf(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  PDF
                </button>
                <button
                  onClick={() => { handleExportMarkdown(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  Markdown
                </button>
                <button
                  onClick={() => { handleCopyMarkdown(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Copy className="w-4 h-4" />
                  Copy Full Transcript
                </button>
              </div>
            )}
          </div>
        </div>
```

**Step 5: Update the `isFileGone` header (line 243-250)**

Replace the Resume button in the file-gone header with Continue This Chat:

```tsx
          {/* Current (lines 243-250): */}
          <button
            onClick={handleResume}
            aria-label="Copy resume command to clipboard"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-blue-500 dark:border-blue-400 text-blue-700 dark:text-blue-300 bg-white dark:bg-gray-800 rounded-md transition-colors hover:bg-blue-50 dark:hover:bg-blue-900/30 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <Terminal className="w-4 h-4" />
            <span>Resume</span>
          </button>

          {/* NOTE: Keep this as Resume for isFileGone state. */}
          {/* When the JSONL file is gone, we can't generate resume context */}
          {/* because useSession (which loads messages) will fail. */}
          {/* The Resume command is the only option in this state. */}
```

**Wait — this is a problem.** In the `isFileGone` state, `session` is null (the JSONL is gone), so `handleContinueChat` would early-return. But `sessionDetail` is available (from the DB). We have two choices:

1. Keep "Resume" button in isFileGone state (it won't work either for orphaned worktrees, but at least it's honest)
2. Build a DB-only version of resume context that doesn't need messages

**Decision:** Keep "Resume" for isFileGone. The full "Continue This Chat" requires messages. This is an edge case (deleted JSONL files) — don't over-engineer.

**Step 6: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "feat: restructure export toolbar — primary Continue button + overflow menu"
```

---

## Task 4: Test manually

**Step 1: Start dev server**

```bash
bun run dev
```

**Step 2: Open a session with data**

Navigate to any session in the browser. Verify:
- [ ] "Continue This Chat" button is visible and blue-styled
- [ ] "Export ▾" dropdown opens on click
- [ ] Dropdown closes on click-outside
- [ ] Dropdown closes after selecting an item
- [ ] Each dropdown item works (Resume, HTML, PDF, MD, Copy)
- [ ] "Continue This Chat" copies text to clipboard
- [ ] Paste the clipboard content — verify it's a structured context prompt, not a raw transcript
- [ ] Cmd+Shift+R triggers Continue This Chat (not Resume)
- [ ] Button is disabled while session is loading (exportsReady = false)

**Step 3: Test edge cases**

- [ ] Session with no `summary` — should fall back to `preview`
- [ ] Session with no files edited — "Files modified" section should be absent (not empty)
- [ ] Session with 0 messages (degenerate) — should still produce project context
- [ ] isFileGone state — should still show Resume button (not Continue)

**Step 4: Build check**

```bash
bun run build
```

Verify no TypeScript errors.

---

## Edge Cases & Fallback Strategy

| Scenario | `summary` | `preview` | Behavior |
|----------|-----------|-----------|----------|
| Normal session | "Refactored auth" | "Can you help me..." | Uses `summary` |
| No summary | null | "Fix the login bug" | Uses `preview` |
| Both empty | null | "" | Skips "What I was doing" section entirely |
| No files edited | — | — | Skips "Files modified" section |
| No files read | — | — | Skips "Files referenced" section |
| Very long conversation (1000+ msgs) | — | — | Only takes last 10 conversational messages |
| isFileGone (JSONL deleted) | — | — | Falls back to Resume button (can't generate context without messages) |

---

## What This Does NOT Do (explicit scope boundaries)

1. **No LLM-based summarization** — uses only structured data already in SessionDetail + last N messages
2. **No new API endpoints** — uses existing `useSession()` + `useSessionDetail()` hooks
3. **No new npm dependencies** — uses existing Lucide icons + existing utils
4. **No session recovery/migration** — doesn't move files in `~/.claude/`
5. **No changes to the session list/cards** — only changes the ConversationView detail header

---

## Rollback

All changes are in 2 files:
- `src/lib/export-markdown.ts` — revert the `generateResumeContext` function
- `src/components/ConversationView.tsx` — revert to the 5-button layout

```bash
git revert HEAD~3..HEAD  # reverts the 3 commits from tasks 1-3
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | Message truncation breaks mid-code-block at 200 chars | Important | Added `truncateAtSafePoint()` helper that avoids breaking inside code blocks and truncates at word boundaries |
| 2 | Misleading comment on `summary` field | Minor | Updated comment to clarify `summary` can be `null`, not just missing |
| 3 | Missing `aria-hidden="true"` on decorative ChevronDown icon | Minor | Added `aria-hidden="true"` to ChevronDown in export dropdown |
| 4 | Empty conversation produces misleading "continue from where we left off" | Minor | Added conditional: shows "help me with this project" when no conversational messages exist |
| 5 | Task 2 showed two conflicting approaches (dynamic import → "Actually, reconsider" → static import) — executor would implement wrong one first | Warning | Removed exploration path, kept only the final static import approach. Renumbered steps. |
| 6 | Task 3 Step 2 misleadingly said "already imported — verify" about `useRef` | Minor | Reworded to clearly state `useRef` is NOT currently imported |
| 7 | Task 4 Step 5 committed `git add -A` but no files changed — would fail or create empty commit | Warning | Removed the bogus commit step from Task 4 |
| 8 | isFileGone Resume button line range said "243-249" but actual code ends at line 250 | Minor | Updated to "243-250" |
| 9 | Task 1 Step 1 code block included `import type { SessionDetail }` inline — pasting verbatim would put an import in the middle of the file (compile error). Conflicts with Step 2 which correctly places it at the top. | Important | Removed the import from the Step 1 code block, added note directing executor to Step 2 for the import |
