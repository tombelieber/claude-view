---
status: pending
date: 2026-02-12
---

# Resume Session Button — Copy Terminal Command

> **Goal:** One-click copy of `claude --resume <id>` from the session detail page, so users can pick up any conversation in their terminal instantly.

## Context

- Phase F (full in-browser resume via Agent SDK sidecar) is designed but deferred
- This is the **bridge UX**: zero-effort way to resume from the dashboard today
- Pattern: same as the existing "Copy" (markdown) button — clipboard + toast

## Design

### Header Layout (Current → Proposed)

```
CURRENT:
┌──────────────────────────────────────────────────────────────────────────┐
│  ← Sessions │ project-name     [Smart][Full]     [HTML↓][PDF↓][MD↓][Copy]│
└──────────────────────────────────────────────────────────────────────────┘

PROPOSED:
┌──────────────────────────────────────────────────────────────────────────┐
│  ← Sessions │ project-name     [Smart][Full]  [▶ Resume] [HTML↓]...[Copy]│
└──────────────────────────────────────────────────────────────────────────┘
                                                 ▲
                                                 └── NEW: visually distinct
```

### The "Resume" Button

**Placement:** Right-aligned actions group, **first** button (before export buttons). Resume is a higher-intent action than export — it deserves the leftmost (most prominent) position in the actions row.

**Visual treatment:** Stands out from the export group with a subtle accent:

```
┌─────────────────────┐   ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐
│  ▶  Resume           │   │ HTML │ │ PDF  │ │ MD   │ │ Copy │
│  (accent border)     │   │  ↓   │ │  ↓   │ │  ↓   │ │  📋  │
└─────────────────────┘   └──────┘ └──────┘ └──────┘ └──────┘
  ↑ blue-600 border         ↑ gray border (existing)
  + Terminal icon
```

**Styling:**
- Border: `border-blue-500 dark:border-blue-400` (vs gray for export buttons)
- Text: `text-blue-700 dark:text-blue-300`
- Icon: `Terminal` from lucide-react (not Play — Terminal is more accurate)
- Hover: `hover:bg-blue-50 dark:hover:bg-blue-900/30`
- Same size/padding as export buttons for alignment

**Why accent border, not filled?** Filled blue would dominate the header. A border accent says "I'm special" without shouting. Consistent with the existing button shape language.

### Click Behavior

1. Click → copies to clipboard:
   ```
   cd /Users/user/dev/@myorg/claude-view && claude --resume 1f319a08-30b1-4623-87de-f7c8237eabed
   ```
2. Toast appears: **"Resume command copied — paste in terminal"** (3s duration)
3. Button text briefly changes to **"Copied!"** with a check icon (1.5s), then reverts

**Why include `cd`?** Sessions are project-scoped. If the user's terminal is in a different directory, `claude --resume` alone may not find the session. The `cd &&` prefix makes it paste-and-go from anywhere.

### Toast Design

```
┌──────────────────────────────────────────────────┐
│  ✓  Resume command copied — paste in terminal    │
└──────────────────────────────────────────────────┘
    ↑ Uses existing showToast() — no new component
```

### Keyboard Shortcut

`Cmd+Shift+R` (Mac) / `Ctrl+Shift+R` (Windows/Linux) — follows the existing pattern of `Cmd+Shift+E` (HTML) and `Cmd+Shift+P` (PDF).

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| Session JSONL deleted (file-gone state) | Button still works — only needs session ID + project path from DB |
| Session has no project path | Omit the `cd` prefix, just copy `claude --resume <id>` |
| Clipboard API blocked by browser | Toast: "Failed to copy — check browser permissions" (same as Copy MD) |

## Implementation

### Files to Change

| File | Change |
|------|--------|
| `src/components/ConversationView.tsx` | Add Resume button + handler + keyboard shortcut |

That's it. One file. Uses existing `showToast()`, existing `copyToClipboard()` from `lib/export-markdown`, and `Terminal` icon from lucide-react.

### Code Sketch

```tsx
// Handler
const handleResume = useCallback(async () => {
  const cmd = projectDir
    ? `cd ${projectDir} && claude --resume ${sessionId}`
    : `claude --resume ${sessionId}`
  const ok = await copyToClipboard(cmd)
  showToast(
    ok ? 'Resume command copied — paste in terminal' : 'Failed to copy — check browser permissions',
    ok ? 3000 : 3000
  )
}, [projectDir, sessionId])

// In the keyboard shortcut handler, add:
if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'r') {
  e.preventDefault()
  handleResume()
}
```

### Button JSX

```tsx
<button
  onClick={handleResume}
  aria-label="Copy resume command to clipboard"
  className="flex items-center gap-2 px-3 py-1.5 text-sm
    border border-blue-500 dark:border-blue-400
    text-blue-700 dark:text-blue-300
    bg-white dark:bg-gray-800 rounded-md transition-colors
    hover:bg-blue-50 dark:hover:bg-blue-900/30
    focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
>
  <Terminal className="w-4 h-4" />
  <span>Resume</span>
</button>
```

## Future

When Phase F lands, this button becomes a **dropdown**:
- Click → opens in-browser resume (Phase F)
- Dropdown arrow → "Copy terminal command" (this feature)

No throwaway work — the button, handler, and shortcut all survive the Phase F upgrade.
