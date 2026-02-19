---
status: done
date: 2026-02-16
---

# Monitor Mode: Rich-Only + Verbose Toggle

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace xterm.js with RichPane as the sole monitor mode renderer, add a verbose toggle (default: chat only), and filter out command tags/noise.

**Architecture:** Monitor mode reads JSONL (structured data) — HTML rendering via react-markdown is strictly better than terminal emulation for this data source. xterm.js is preserved in the codebase for Phase F (Interactive Control) where we'll own the PTY via Agent SDK. A global "verbose" toggle in GridControls controls whether tool calls, thinking, and tool results are shown.

**Tech Stack:** React, Zustand, react-markdown (existing), Rust server-side content filtering

---

## Context: Why This Change

The current monitor mode offers a raw/rich toggle per pane. The "raw" mode pipes JSONL lines into xterm.js, which shows unformatted JSON — useless for monitoring. The "rich" mode (RichPane) already renders markdown, but shows ALL message types including tool calls, thinking blocks, and raw command tags like `<command-name>/clear</command-name>`.

**Key insight:** We're reading JSONL log files, not a PTY stream. xterm.js adds complexity (WebGL contexts, ANSI rendering) with no benefit for structured data. RichPane + react-markdown gives native HTML tables, bold, code blocks — exactly what Claude Code's output looks like.

**xterm.js stays for Phase F** where we spawn new sessions from the web UI and own the PTY output stream — Claude Code formats its own output with ANSI codes.

---

### Task 1: Update monitor-store — Replace paneMode with verboseMode

**Files:**
- Modify: `src/store/monitor-store.ts`
- Test: `src/store/monitor-store.test.ts`

**Step 1: Write the failing test**

In `src/store/monitor-store.test.ts`, add a test for verbose mode:

```ts
it('verboseMode defaults to false', () => {
  const state = useMonitorStore.getState()
  expect(state.verboseMode).toBe(false)
})

it('toggleVerbose flips verboseMode', () => {
  useMonitorStore.getState().toggleVerbose()
  expect(useMonitorStore.getState().verboseMode).toBe(true)
  useMonitorStore.getState().toggleVerbose()
  expect(useMonitorStore.getState().verboseMode).toBe(false)
})
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run src/store/monitor-store.test.ts`
Expected: FAIL — `verboseMode` and `toggleVerbose` don't exist yet.

**Step 3: Update the store**

In `src/store/monitor-store.ts`:

1. Remove `paneMode: Record<string, 'raw' | 'rich'>` from state
2. Remove `setPaneMode` action
3. Add `verboseMode: boolean` (default: `false`)
4. Add `toggleVerbose: () => void` action
5. Update `partialize` to persist `verboseMode` instead of `paneMode`

Also update the test file:
- In `beforeEach` reset block: remove `paneMode: {}` and add `verboseMode: false` to the reset state object. This ensures test isolation — without it, tests that call `toggleVerbose()` will leak state into subsequent tests.
- Remove the `'has empty paneMode record'` test (lines 43-44) — `paneMode` no longer exists.
- Remove the entire `describe('setPaneMode', ...)` block (lines 187-209) — 3 tests for the removed action.

```ts
// Remove from interface and implementation:
// paneMode: Record<string, 'raw' | 'rich'>
// setPaneMode: (id: string, mode: 'raw' | 'rich') => void

// Add:
verboseMode: false,
toggleVerbose: () => set((state) => ({ verboseMode: !state.verboseMode })),
```

**Step 4: Run test to verify it passes**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run src/store/monitor-store.test.ts`
Expected: PASS

**Step 5: Commit**

```bash
git add src/store/monitor-store.ts src/store/monitor-store.test.ts
git commit -m "feat(monitor): replace paneMode with verboseMode in store"
```

---

### Task 2: Add verbose toggle to GridControls

**Files:**
- Modify: `src/components/live/GridControls.tsx`
- Test: `src/components/live/GridControls.test.tsx`

**Step 1: Update GridControlsProps**

Add new props:

```ts
interface GridControlsProps {
  // ... existing props ...
  verboseMode: boolean
  onVerboseModeChange: () => void
}
```

**Step 2: Add verbose toggle button**

Add after the Compact toggle button, before the flex spacer (`<div className="flex-1" />`). Insert a new separator + the verbose toggle button. The resulting order is: Compact toggle → Separator → Verbose toggle → Spacer → Session count badge.

```tsx
{/* Separator */}
<div className="h-4 w-px bg-gray-200 dark:bg-gray-700" />

{/* Verbose toggle */}
<button
  type="button"
  onClick={onVerboseModeChange}
  className={cn(
    'flex items-center gap-1 px-2 py-1 rounded-md text-xs font-medium transition-colors',
    verboseMode
      ? 'bg-indigo-500/10 text-indigo-400 border border-indigo-500/30'
      : 'text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-200/50 dark:hover:bg-gray-700/50 border border-transparent'
  )}
  aria-pressed={verboseMode}
  title={verboseMode ? 'Showing all messages (tool calls, thinking, etc.)' : 'Showing chat only (user + assistant)'}
>
  {verboseMode ? (
    <ListTree className="h-3 w-3" />
  ) : (
    <MessageSquare className="h-3 w-3" />
  )}
  {verboseMode ? 'Verbose' : 'Chat'}
</button>
```

Import `ListTree` and `MessageSquare` from `lucide-react`.

**Step 3: Update test**

In `GridControls.test.tsx`:
1. Update the `defaultProps` / `renderGridControls` helper to include the new required props: `verboseMode: false` and `onVerboseModeChange: vi.fn()`. Without this, ALL existing tests will fail with a TypeScript type error after adding the required props.
2. Add a new test verifying the verbose button renders with correct label ("Chat" when off, "Verbose" when on) and fires the `onVerboseModeChange` callback when clicked.

**Step 4: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run src/components/live/GridControls.test.tsx`

**Step 5: Commit**

```bash
git add src/components/live/GridControls.tsx src/components/live/GridControls.test.tsx
git commit -m "feat(monitor): add verbose toggle to GridControls toolbar"
```

---

### Task 3: Add content filtering to RichPane

**Files:**
- Modify: `src/components/live/RichPane.tsx`

**Step 1: Add command tag stripping**

Add a `stripCommandTags` function that removes Claude Code internal markup from message content:

```ts
/** Strip Claude Code internal command tags from content.
 * These tags appear in JSONL but are not meant for display:
 * <command-name>...</command-name>
 * <command-message>...</command-message>
 * <command-args>...</command-args>
 * <local-command-stdout>...</local-command-stdout>
 */
function stripCommandTags(content: string): string {
  return content
    .replace(/<command-name>[\s\S]*?<\/command-name>/g, '')
    .replace(/<command-message>[\s\S]*?<\/command-message>/g, '')
    .replace(/<command-args>[\s\S]*?<\/command-args>/g, '')
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, '')
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .trim()
}
```

Apply `stripCommandTags()` in `parseRichMessage` to the `content` value in each return statement. There are 6 return paths — modify 4, skip 2:

| Line | Type | Action | Why |
|------|------|--------|-----|
| 46 | message | `content: stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content))` | Strip tags from assistant/user messages |
| 53 | tool_use | **Skip** — content is already `''` | Empty string, nothing to strip |
| 62 | tool_result | `content: stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content \|\| ''))` | Tool results can contain `<local-command-stdout>` |
| 69 | thinking | `content: stripCommandTags(typeof msg.content === 'string' ? msg.content : '')` | Thinking blocks can contain `<system-reminder>` |
| 76 | error | **Skip** — content is `msg.message`, not Claude output | Error descriptions are internal, not tag-contaminated |
| 82 | line | `content: stripCommandTags(typeof msg.data === 'string' ? msg.data : '')` | Raw line data may contain tags |

**Step 2: Add verbose filtering to RichPane**

Add `verboseMode` prop to `RichPaneProps`:

```ts
export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  followOutput?: boolean
  verboseMode?: boolean  // NEW: when false (default), only show user + assistant
}
```

Add `useMemo` to the React import on line 1:
```ts
import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
```

Destructure `verboseMode` from props on line 242:
```ts
export function RichPane({ messages, isVisible, followOutput: followOutputProp = true, verboseMode = false }: RichPaneProps) {
```

Add the filter immediately after the destructuring (before `const virtuosoRef`):
```ts
const displayMessages = useMemo(() => {
  if (verboseMode) return messages
  return messages.filter((m) => m.type === 'user' || m.type === 'assistant' || m.type === 'error')
}, [messages, verboseMode])
```

Replace ALL `messages` references used for Virtuoso rendering/interaction with `displayMessages`. There are **8 locations** — missing any one causes either a runtime scroll error or a stale closure:

```tsx
// 1. (line 246) Ref initialization
const prevMessageCountRef = useRef(displayMessages.length)

// 2. (line 250) New message detection — if condition
if (displayMessages.length > prevMessageCountRef.current) {

// 3. (line 257) Ref update
prevMessageCountRef.current = displayMessages.length

// 4. (line 258) useEffect dependency array — triggers re-check when filter changes
}, [displayMessages.length, isAtBottom])

// 5. (line 269) scrollToIndex inside scrollToBottom callback
index: displayMessages.length - 1

// 6. (line 273) scrollToBottom dependency array — prevents stale closure on verbose toggle
}, [displayMessages.length])

// 7. (line 277) Empty state check
if (displayMessages.length === 0)

// 8. (line 289) Virtuoso data prop
data={displayMessages}
```

This prevents runtime scroll errors when verbose mode is off (e.g., `scrollToIndex({ index: 49 })` on a Virtuoso list with only 20 visible items) and prevents stale closures when verbose mode toggles.

**Step 3: Filter empty messages after tag stripping**

After stripping command tags, some messages become empty (e.g., a message that was only `<command-name>/clear</command-name>`). Filter these out in `parseRichMessage`:

For each return path where `stripCommandTags` was applied (message, tool_result, thinking, line), add an emptiness check AFTER stripping:

```ts
// Example for the message return path (line 44):
if (msg.type === 'message') {
  const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content))
  if (!content.trim()) return null  // Empty after stripping → drop
  return { type: msg.role === 'user' ? 'user' : 'assistant', content, ts: msg.ts }
}
```

**Do NOT add the emptiness check for:**
- `tool_use` — content is intentionally `''` (tool name/input are in separate fields)
- `error` — error messages should always display, even if short

**Step 4: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run src/components/live/`

**Step 5: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat(monitor): add command tag stripping and verbose filtering to RichPane"
```

---

### Task 4: Remove raw mode from MonitorView, MonitorPane, PaneContextMenu, and ExpandedPaneOverlay (ATOMIC)

> **IMPORTANT:** Tasks 4, 5, and 6 from the original plan are merged into this single atomic task. The reason: `mode` and `onModeToggle`/`onToggleMode` are **required** props in `MonitorPaneProps` and `PaneContextMenuProps`. Removing call sites (MonitorView) before interfaces (MonitorPane, PaneContextMenu) causes a TypeScript "missing required prop" error. Removing interfaces before call sites causes a "property does not exist" error. Both must be done in the same commit.

**Files:**
- Modify: `src/components/live/MonitorView.tsx`
- Modify: `src/components/live/MonitorPane.tsx`
- Modify: `src/components/live/MonitorPane.test.tsx`
- Modify: `src/components/live/PaneContextMenu.tsx`
- Modify: `src/components/live/PaneContextMenu.test.tsx`
- Modify: `src/components/live/ExpandedPaneOverlay.tsx`

**Step 1: Remove raw mode references from MonitorView**

Remove ALL `paneMode` and `setPaneMode` references in `MonitorView.tsx`:
1. Remove `import { TerminalPane } from './TerminalPane'`
2. Remove `const paneMode = useMonitorStore((s) => s.paneMode)` (~line 68)
3. Remove `const setPaneMode = useMonitorStore((s) => s.setPaneMode)` (~line 78)
4. Remove `handleModeToggle` callback (~lines 160-166)
5. Add `const verboseMode = useMonitorStore((s) => s.verboseMode)` and `const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)` from store
6. Remove `const mode = paneMode[session.id] || 'rich'` local variable in grid rendering (~line 222)
7. Remove `mode` and `onModeToggle` props from `<MonitorPane>` call site (~lines 244, 249)
8. Remove `mode` and `onToggleMode` props from `<PaneContextMenu>` call site (~lines 302, 326-328)
9. Remove `mode={paneMode[expandedSession.id] || 'rich'}` from `<ExpandedPaneOverlay>` (~line 277)

**Step 2: Update RichTerminalPane to accept verboseMode**

```tsx
function RichTerminalPane({ sessionId, isVisible, verboseMode }: {
  sessionId: string
  isVisible: boolean
  verboseMode: boolean
}) {
  // ... existing logic ...
  return <RichPane messages={messages} isVisible={isVisible} followOutput={bufferLoaded} verboseMode={verboseMode} />
}
```

**Step 3: Replace the raw/rich conditional with RichTerminalPane only**

In the pane rendering, replace:
```tsx
{mode === 'raw' ? (
  <TerminalPane sessionId={session.id} mode="raw" isVisible={isPaneVisible} />
) : (
  <RichTerminalPane sessionId={session.id} isVisible={isPaneVisible} />
)}
```

With:
```tsx
<RichTerminalPane sessionId={session.id} isVisible={isPaneVisible} verboseMode={verboseMode} />
```

**Step 4: Wire verbose toggle to GridControls**

```tsx
<GridControls
  // ... existing props ...
  verboseMode={verboseMode}
  onVerboseModeChange={toggleVerbose}
/>
```

**Step 5: Update ExpandedPaneOverlay rendering in MonitorView**

The raw/rich conditional for the expanded pane lives in **MonitorView.tsx** (not inside ExpandedPaneOverlay.tsx itself). Around lines 280-291 of MonitorView, replace the `mode === 'raw' ? <TerminalPane .../> : <RichTerminalPane .../>` ternary with just `<RichTerminalPane ... verboseMode={verboseMode} />`.

Also remove the `mode` prop passed to `<ExpandedPaneOverlay>` and remove the mode label display inside `ExpandedPaneOverlay.tsx` (around line 179-182).

**Step 6: Remove mode from MonitorPane interface and UI**

In `src/components/live/MonitorPane.tsx` — remove ALL 12 mode references:

Interface (2 removals):
1. Remove `mode: 'raw' | 'rich'` from `MonitorPaneProps` interface (line 84)
2. Remove `onModeToggle: () => void` from `MonitorPaneProps` interface (line 89)

Component destructuring (2 removals):
3. Remove `mode` from props destructuring (line 103)
4. Remove `onModeToggle` from props destructuring (line 108)

Props passed to FullHeader (2 removals):
5. Remove `mode={mode}` prop (line 168)
6. Remove `onModeToggle={onModeToggle}` prop (line 170)

FullHeader function params (2 removals):
7. Remove `mode` param (line 201)
8. Remove `onModeToggle` param (line 203)

FullHeader type annotations (2 removals):
9. Remove `mode: 'raw' | 'rich'` type (line 213)
10. Remove `onModeToggle: () => void` type (line 215)

FullHeader body (1 removal):
11. Remove mode toggle button (lines 276-289) — the entire `<button>` with `onModeToggle()` and Terminal/MessageSquare icons

Imports (1 removal):
12. Remove `Terminal` and `MessageSquare` from lucide-react imports (lines 9-10) — only used by the mode toggle button. Note: `MessageSquare` is separately imported in GridControls.tsx (added in Task 2); that is a different file and unrelated.

In `src/components/live/MonitorPane.test.tsx` — remove ALL 4 mode references:
1. Remove `mode: 'raw'` from defaultProps (line 57)
2. Remove `onModeToggle: vi.fn()` from defaultProps (line 62)
3. Remove `mode='raw'` from explicit test props (line 242)
4. Remove `onModeToggle={vi.fn()}` from explicit test props (line 247)
Note: There are no dedicated mode toggle tests in this file, only props.

**Step 7: Remove mode from PaneContextMenu interface**

In `src/components/live/PaneContextMenu.tsx` — remove ALL 6 mode references:

Interface (2 removals):
1. Remove `mode: 'raw' | 'rich'` from `PaneContextMenuProps` (line 18)
2. Remove `onToggleMode: () => void` from `PaneContextMenuProps` (line 25)

Component destructuring (2 removals):
3. Remove `mode` from props destructuring (line 42)
4. Remove `onToggleMode` from props destructuring (line 49)

Menu item (1 removal):
5. Remove the "Switch to Raw/Rich" menu item conditional (lines 66-70)

Imports (1 removal):
6. Remove `Terminal` and `MessageSquare` from lucide-react imports (line 9) — only used by the mode menu item being removed

In `src/components/live/PaneContextMenu.test.tsx` — 7 specific changes:

Props (2 removals):
1. Remove `mode: 'raw'` from defaultProps (line 11)
2. Remove `onToggleMode: vi.fn()` from defaultProps (line 18)

Count assertions (2 changes):
3. Change `toHaveLength(5)` → `toHaveLength(4)` (line 31 — "renders all N menu items when not pinned")
4. Change `toHaveLength(5)` → `toHaveLength(4)` (line 191 — ARIA "has role=menuitem on each item")

Test removals (3 deletions):
5. Remove test "renders Switch to Rich when mode is raw" (lines 66-71)
6. Remove test "renders Switch to Raw when mode is rich" (lines 73-78)
7. Remove test "calls onToggleMode and onClose when Switch to Rich is clicked" (lines 137-146)

**Step 8: Remove mode from ExpandedPaneOverlay**

In `src/components/live/ExpandedPaneOverlay.tsx` — 3 removals:
1. Remove `mode: 'raw' | 'rich'` from `ExpandedPaneOverlayProps` interface (line 72)
2. Remove `mode` from props destructuring (line 81)
3. Remove the mode label display (lines 179-182)

**Step 9: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run src/components/live/`

Verify all monitor component tests pass (MonitorPane, PaneContextMenu, GridControls).

**Step 10: Run the app and verify**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bun run dev`

Navigate to Mission Control → Monitor view. Verify:
- All panes show RichPane (no xterm.js)
- Verbose toggle appears in toolbar
- Default: only user + assistant messages shown
- Verbose ON: tool calls, thinking, results also shown
- No `<command-name>` tags visible
- No mode toggle button in pane headers
- No "Switch to Raw/Rich" in right-click context menu

**Step 11: Commit (ATOMIC — all mode removal in one commit)**

```bash
git add src/components/live/MonitorView.tsx src/components/live/ExpandedPaneOverlay.tsx src/components/live/MonitorPane.tsx src/components/live/MonitorPane.test.tsx src/components/live/PaneContextMenu.tsx src/components/live/PaneContextMenu.test.tsx
git commit -m "feat(monitor): remove xterm.js and raw/rich mode, use RichPane exclusively with verbose toggle"
```

---

### Task 5: Update keyboard shortcuts — Repurpose 'M' key

**Files:**
- Modify: `src/components/live/useMonitorKeyboardShortcuts.ts`

**Step 1: Update the docstring**

Change the keyboard shortcuts table in the JSDoc comment (line 37):
```
// Before:
// | m       | Toggle raw/rich mode on selected pane        |
// After:
// | m       | Toggle verbose mode (global)                 |
```

**Step 2: Change 'm' from mode toggle to verbose toggle**

Replace the entire `case 'm':` block (lines 119-127). The current code has a `selectedId` guard and calls `store.setPaneMode()`. Replace with:

```ts
case 'm': {
  store.toggleVerbose()
  e.preventDefault()
  break
}
```

Notes:
- Uses `store.toggleVerbose()` (not `useMonitorStore.getState()`) — matches all other cases in this switch which use the pre-captured `store` variable (line 50).
- Block-scoped with braces `{ }` — matches all other cases in this switch.
- `e.preventDefault()` added to match the pattern of every other action case.
- No `selectedId` guard needed since verbose mode is global (not per-pane).
- Removes the `const selectedId` and `const currentMode` local variables that were inside the old case.

**Step 3: Commit**

```bash
git add src/components/live/useMonitorKeyboardShortcuts.ts
git commit -m "refactor(monitor): repurpose M key from mode toggle to verbose toggle"
```

---

### Task 6: Server-side — Filter command tags in rich mode

**Files:**
- Modify: `crates/server/src/routes/terminal.rs`

**Step 1: Add command tag stripping in format_line_for_mode**

After extracting text content in rich mode, strip command tags before sending:

```rust
/// Strip Claude Code internal command tags from content.
fn strip_command_tags(content: &str) -> String {
    let mut result = content.to_string();
    let tags = ["command-name", "command-message", "command-args",
                "local-command-stdout", "system-reminder"];

    for tag in &tags {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");

        // Loop until no more opening tags are found
        while let Some(start) = result.find(&open) {
            // Search for closing tag AFTER the opening tag position
            match result[start..].find(&close) {
                Some(offset) => {
                    let end = start + offset + close.len();
                    result.replace_range(start..end, "");
                }
                None => {
                    // No closing tag found — break to avoid infinite loop
                    break;
                }
            }
        }
    }
    result.trim().to_string()
}
```

Apply `strip_command_tags` at **both** text emission locations in `format_line_for_mode`:

1. **String content path** (~line 274): After extracting `content` as a `String`, call `let stripped = strip_command_tags(s);` and use `stripped` in the JSON output. If `stripped.is_empty()`, return `vec![]`.
2. **Concatenated text blocks path** (~line 367): After joining `text_parts`, call `let stripped = strip_command_tags(&full_text);` and use `stripped`. If `stripped.is_empty()`, skip the `results.push(...)`.

**Step 2: Skip messages that become empty after stripping**

At both emission locations above, add an emptiness check after stripping. If the stripped content is empty or only whitespace, don't emit the message (return `vec![]` for the string path, skip the push for the text blocks path).

**Step 3: Add unit tests inside the existing `#[cfg(test)] mod tests` block**

Add these tests **at the end of** the existing `#[cfg(test)] mod tests { ... }` block in `terminal.rs` (search for `mod tests` — it starts around line 785). Place them just before the closing `}` of the `tests` module. Do NOT create a new test module:

```rust
    #[test]
    fn strip_command_tags_removes_all_known_tags() {
        let input = r#"<command-name>/clear</command-name>
<command-message>clear</command-message>
<command-args></command-args>

NaN ago
<local-command-stdout></local-command-stdout>"#;
        let result = strip_command_tags(input);
        assert!(!result.contains("<command-name>"));
        assert!(!result.contains("<local-command-stdout>"));
        // After stripping all tags and trimming, only "NaN ago" should remain
        assert_eq!(result, "NaN ago");
    }

    #[test]
    fn strip_command_tags_preserves_normal_content() {
        let input = "Here is a table:\n\n| Col1 | Col2 |\n|------|------|\n| a    | b    |";
        let result = strip_command_tags(input);
        assert_eq!(result, input);
    }

    #[test]
    fn strip_command_tags_handles_missing_close_tag() {
        let input = "<command-name>unclosed content but normal text after";
        let result = strip_command_tags(input);
        // Should not infinite loop; returns input unchanged since no closing tag
        assert_eq!(result, input);
    }
```

**Step 4: Run tests**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && cargo test -p vibe-recall-server -- routes::terminal`

**Step 5: Commit**

```bash
git add crates/server/src/routes/terminal.rs
git commit -m "feat(server): strip command tags from rich mode content"
```

---

### Task 7: Update plan files — Reflect architectural change

**Files:**
- Modify: `docs/plans/mission-control/design.md`
- Modify: `docs/plans/mission-control/phase-c-monitor-mode.md`
- Modify: `docs/plans/mission-control/PROGRESS.md`

**Step 1: Update design.md**

Line 62: Change `| **Monitor mode** | Live terminal output grid via xterm.js | C |` to:
`| **Monitor mode** | Live session chat grid via RichPane (HTML) | C |`

Line 127: Change `| **WebSocket Endpoint** | `server` | Streams raw terminal output bytes for Monitor mode...` to:
`| **WebSocket Endpoint** | `server` | Streams structured JSONL messages for Monitor mode. One WebSocket per monitored pane. Rich mode parses messages into user/assistant/tool/thinking types. |`

Line 250: Remove or annotate xterm dependency:
`| `xterm` | 5.x | ~150KB | Terminal emulator — **deferred to Phase F (Interactive Control)** |`

Line 887: Update monitor mode description:
`Live session chat grid. Each pane shows a read-only chat view (RichPane) displaying the conversation for one session, with markdown rendering for tables, code blocks, etc.`

Line 916: Update sub-modes section to replace raw/rich with verbose/chat.

Lines 1098-1107: Update WebSocket description to note structured messages instead of "raw terminal bytes".

**Step 2: Update phase-c-monitor-mode.md**

1. Add a note at the top under the YAML frontmatter: `> **Architectural update (2026-02-16):** Monitor mode uses RichPane (HTML) exclusively. xterm.js is deferred to Phase F (Interactive Control) where we own the PTY. See `docs/plans/2026-02-16-monitor-rich-only.md` for rationale.`

2. In Step 2 (xterm.js Integration): Mark the entire section as "Deferred to Phase F". The WebSocket hook and infrastructure remain, but TerminalPane is not used in monitor mode.

3. In Step 5 (Rich vs Raw Toggle): Replace with "Verbose Toggle" — chat-only (default) vs. full details (tool calls, thinking, etc.).

4. Update acceptance criteria:
   - AC-4: ~~xterm.js renders~~ → "RichPane renders markdown correctly (tables, bold, code blocks)"
   - AC-10: ~~Rich/Raw toggle~~ → "Verbose toggle shows/hides tool calls, thinking, and results"

**Step 3: Update PROGRESS.md**

Add a key decision to the Key Decisions Log:

```
| 2026-02-16 | **Monitor mode uses RichPane (HTML) exclusively — no xterm.js.** xterm.js deferred to Phase F (Interactive Control) where we own the PTY via Agent SDK. Monitor mode reads JSONL (structured data) → HTML rendering is strictly better. Verbose toggle replaces raw/rich toggle. | Existing sessions run in VS Code/terminal — we can't tap their PTY. Our only interface is JSONL log files. HTML renders markdown (tables, bold, code) better than terminal ANSI conversion. |
```

Update Phase C description in the At a Glance table:
`| C | Monitor Mode | `in-progress` | Live chat grid, WebSocket + RichPane (HTML), verbose toggle, responsive pane grid |`

**Step 4: Commit**

```bash
git add docs/plans/mission-control/design.md docs/plans/mission-control/phase-c-monitor-mode.md docs/plans/mission-control/PROGRESS.md docs/plans/2026-02-16-monitor-rich-only.md
git commit -m "docs: update plans to reflect RichPane-only monitor architecture"
```

---

### Task 8: Move xterm-specific files into terminal/ subdirectory

**Files:**
- Move: `src/components/live/TerminalPane.tsx` → `src/components/live/terminal/TerminalPane.tsx`
- Create: `src/components/live/terminal/index.ts` (re-export)
- Move: `src/hooks/use-terminal-autoscroll.ts` → `src/hooks/terminal/use-terminal-autoscroll.ts`
- Create: `src/hooks/terminal/index.ts` (re-export)

**Why:** Preserve the xterm.js UI code for Phase F (Interactive Control) without cluttering the active monitor components. The `terminal/` subdirectory makes it clear these are dormant-but-intentional, not dead code.

**What stays in place (shared by both modes):**
- `src/hooks/use-terminal-socket.ts` — WebSocket hook used by RichTerminalPane
- `src/lib/ws-url.ts` — WebSocket URL helper used everywhere

**Step 1: Create directories and move files**

```bash
mkdir -p src/components/live/terminal
mkdir -p src/hooks/terminal
git mv src/components/live/TerminalPane.tsx src/components/live/terminal/TerminalPane.tsx
git mv src/hooks/use-terminal-autoscroll.ts src/hooks/terminal/use-terminal-autoscroll.ts
```

**Step 2: Create index.ts re-exports**

`src/components/live/terminal/index.ts`:
```ts
export { TerminalPane } from './TerminalPane'
export type { TerminalPaneProps } from './TerminalPane'
```

`src/hooks/terminal/index.ts`:
```ts
export { useTerminalAutoScroll } from './use-terminal-autoscroll'
```

**Step 3: Update any remaining imports**

**Critical:** After moving `TerminalPane.tsx` one level deeper, its internal import `../../hooks/use-terminal-socket` (line 7) resolves to `src/components/hooks/` (wrong) instead of `src/hooks/`. Fix it:

```ts
// Before (line 7 of TerminalPane.tsx):
import { useTerminalSocket, type ConnectionState } from '../../hooks/use-terminal-socket'
// After:
import { useTerminalSocket, type ConnectionState } from '../../../hooks/use-terminal-socket'
```

Similarly, `use-terminal-autoscroll.ts` imports from `@xterm/xterm` (npm package) — this needs no path change.

Also search for `from './TerminalPane'` or `from '../../hooks/use-terminal-autoscroll'` in other files and update paths. Since we removed TerminalPane from MonitorView in Task 4, there should be no live imports — but verify with grep.

**Step 4: Run full test suite**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run`

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: move xterm-specific files to terminal/ subdirs for Phase F"
```

---

### Task 9: Cleanup — Remove unused imports and dead code

**Files:**
- Modify: `src/components/live/MonitorView.tsx` — Remove TerminalPane import if still present
- Modify: `src/components/live/ExpandedPaneOverlay.tsx` — Remove mode/raw references

**Step 1: Verify no remaining raw mode references in monitor components**

Search for `'raw'` and `TerminalPane` in the live components directory (not terminal/ subdir). Remove any dead references.

**Step 2: Run full test suite**

Run: `cd /Users/user/dev/@myorg/claude-view/.worktrees/mission-control-cde && bunx vitest run`

**Step 3: Commit**

```bash
git add -A
git commit -m "chore: remove dead raw mode references from monitor components"
```

---

## Summary of Changes

| Component | Before | After |
|-----------|--------|-------|
| Monitor pane content | xterm.js (raw) or RichPane (rich) per toggle | RichPane only |
| Mode toggle (header button) | Raw ⇄ Rich per pane | Removed |
| Verbose toggle (toolbar) | N/A | Chat-only (default) / Full details |
| Command tags (`<command-name>` etc.) | Shown raw | Stripped server-side + client-side |
| `paneMode` in store | `Record<string, 'raw' \| 'rich'>` | Removed, replaced with `verboseMode: boolean` |
| M keyboard shortcut | Toggle raw/rich | Toggle verbose |
| Context menu "Switch to Raw/Rich" | Present | Removed |
| xterm.js dependency | Used in monitor | Kept in codebase for Phase F |
| TerminalPane component | Used in monitor | Kept in codebase for Phase F |

## Preserved for Phase F (Interactive Control)

**Moved to `terminal/` subdirs (dormant, not dead):**
- `src/components/live/terminal/TerminalPane.tsx` — xterm.js wrapper, will render PTY output
- `src/hooks/terminal/use-terminal-autoscroll.ts` — auto-scroll for xterm

**Shared infra (actively used by both monitor + future interactive):**
- `src/hooks/use-terminal-socket.ts` — WebSocket connection hook
- `src/lib/ws-url.ts` — WebSocket URL helper
- `@xterm/xterm`, `@xterm/addon-fit`, `@xterm/addon-webgl` — npm deps kept

**Rust (serves both modes):**
- `crates/server/src/routes/terminal.rs` — raw mode path preserved, just not used by monitor
- `crates/server/src/terminal_state.rs` — connection limits
- `crates/server/src/file_tracker.rs` — incremental file reader
- `crates/core/src/tail.rs` — efficient tail reader

---

## Rollback

Each task commits separately, so rollback is straightforward:

```bash
# Revert the last N commits (one per task):
git revert HEAD~N..HEAD --no-commit
git commit -m "revert: undo monitor rich-only changes"
```

**Partial rollback:** If only server-side (Task 6) needs reverting, `git revert <task-6-commit>` is safe — the frontend changes don't depend on server-side tag stripping (client-side stripping in Task 3 is redundant defense-in-depth).

**Full rollback priority:** If the feature is broken, revert Tasks 4→5 first (they remove the old UI) before reverting Tasks 1-3 (which add the new UI). Reverting 1-3 without 4-5 leaves the app with no mode toggle AND no verbose toggle.

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `strip_command_tags` loop searched for closing tag from position 0 (not after opening tag), causing incorrect removal or infinite loops | Blocker | Rewrote to use `result[start..].find(&close)` with `match` and `None → break` guard |
| 2 | Unit tests shown as bare `#[test]` functions outside any module | Blocker | Added note to place tests inside existing `#[cfg(test)] mod tests` block; added missing-close-tag edge case test |
| 3 | Plan said "text content emission paths" without specifying the two exact locations | Warning | Added explicit line references (~274 string path, ~367 concatenated path) and described what to change at each |
| 4 | Test `beforeEach` reset includes `paneMode: {}` which must be removed alongside store changes | Warning | Added note in Task 1 Step 3 to update `beforeEach` reset |
| 5 | Plan said "after the separator" but actual layout has a flex spacer, not a separator | Warning | Rewrote placement instruction: after Compact toggle, before flex spacer, with new separator |
| 6 | ExpandedPaneOverlay Step 5 said "replace conditional in overlay" but the conditional lives in MonitorView | Warning | Clarified that conditional is in MonitorView (~lines 280-291), also need to remove `mode` prop from overlay component |
| 7 | Task 4 commit only staged MonitorView.tsx but Step 5 also modifies ExpandedPaneOverlay.tsx | Minor | Added ExpandedPaneOverlay.tsx to `git add` in Step 7 |

### Round 2 (Adversarial Review → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 8 | `scrollToIndex` uses `messages.length` but Virtuoso now has `displayMessages` — runtime scroll error when verbose=off | Critical | Listed all 7 `messages` references that must change to `displayMessages` (data prop, scrollToIndex, prevMessageCountRef ×3, dependency array, empty check) |
| 9 | `beforeEach` reset missing `verboseMode: false` — state leaks between tests | Important | Added `verboseMode: false` to the beforeEach reset instruction |
| 10 | `handleModeToggle` removed in Task 4 but still referenced by PaneContextMenu's `onToggleMode` prop in MonitorView — compile failure between Task 4 and Task 6 | Important | Added step 6 to Task 4 Step 1: remove `onToggleMode` and `mode` props from PaneContextMenu/MonitorPane call sites in MonitorView |
| 11 | GridControls.test.tsx `defaultProps` missing new required props after Task 2 — all existing tests break | Important | Added instruction to update `defaultProps`/helper with `verboseMode: false` and `onVerboseModeChange: vi.fn()` |
| 12 | "Remove Terminal and MessageSquare imports (if no longer used elsewhere)" — ambiguous scope | Important | Scoped to "from this file (MonitorPane.tsx)" with note that GridControls.tsx has its own import |
| 13 | Missing `e.preventDefault()` in Task 7 keyboard shortcut; extra `case 'M':` inconsistent with codebase | Important | Added `e.preventDefault()`, removed `case 'M':` to match existing switch style |
| 14 | `stripCommandTags` application location in `parseRichMessage` was vague ("all content fields before returning") | Suggestion | Specified: apply as post-processing on `content` field just before each return statement |

### Round 3 (Final Adversarial Review → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 15 | `mode`/`onModeToggle`/`onToggleMode` are **required** props in MonitorPane and PaneContextMenu — removing from call sites (Task 4) before removing from interfaces (Tasks 5-6) causes TS compile failure | Critical | **Merged Tasks 4, 5, 6 into single atomic Task 4** with one commit that changes interfaces and call sites simultaneously |
| 16 | `scrollToBottom` callback dependency array `[messages.length]` not updated → stale closure when verbose toggles | Important | Added item 5 to the displayMessages change list: dependency array ~line 273 |
| 17 | MonitorView has 8 `paneMode` references but plan only addressed 5 (missed setPaneMode action ~line 78, local `mode` variable ~line 222, context menu mode prop ~line 302) | Important | Enumerated all 9 removal points explicitly in Task 4 Step 1 |
| 18 | Keyboard shortcut code block used `useMonitorStore.getState()` but actual code uses pre-captured `store` variable; missing block-scope braces | Important | Rewrote to use `store.toggleVerbose()` with `{ }` braces matching codebase convention |
| 19 | Test module placement in terminal.rs ambiguous ("starts around line 785") | Minor | Specified: "at the end of the existing `mod tests` block, just before closing `}`" |
| 20 | Task numbering out of sequence after merging Tasks 5-6 into Task 4 | Minor | Renumbered: old Tasks 7→5, 8→6, 9→7, 10→8, 11→9 (now 9 tasks total) |

### Round 4 (Codebase Verification Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 21 | `useMemo` not imported in RichPane.tsx — plan adds `useMemo(...)` call without mentioning the import | Blocker | Added explicit import instruction: `import { useState, useRef, useEffect, useCallback, useMemo } from 'react'` |
| 22 | 8 `messages`→`displayMessages` locations but plan listed only 7 — missed useEffect dependency array (line 258: `[messages.length, isAtBottom]`) | Blocker | Added item 4 (useEffect dep array), renumbered to 8 items total |
| 23 | PaneContextMenu.test.tsx changes not enumerated — "remove mode tests" is too vague for copy-paste execution | Blocker | Enumerated all 7 specific changes: 2 prop removals, 2 count assertions (5→4), 3 test deletions with exact line numbers |
| 24 | PaneContextMenu.tsx `Terminal`/`MessageSquare` imports not removed after deleting mode menu item — unused imports cause lint errors | Blocker | Added item 6 to PaneContextMenu removal list: remove Terminal/MessageSquare from lucide-react imports |
| 25 | Keyboard shortcuts docstring (line 37) says "Toggle raw/rich mode on selected pane" — stale after Task 5 | Important | Added Step 1 to Task 5: update docstring to "Toggle verbose mode (global)" |
| 26 | MonitorPane.tsx mode removal listed 4 items but actual file has 12 mode references (interface, destructuring, FullHeader params, FullHeader types, FullHeader props, button, imports) | Important | Enumerated all 12 removal points with exact line numbers |
| 27 | `stripCommandTags` application in parseRichMessage said "wrap each return" but parseRichMessage has 6 return paths — unclear which to modify | Important | Added table specifying all 6 paths: modify 4 (message, tool_result, thinking, line), skip 2 (tool_use has empty content, error is not Claude output) |
| 28 | No rollback instructions — audit criterion requires "Plan states how to undo if things go wrong" | Minor | Added Rollback section with `git revert` instructions and partial/full rollback priority |

### Round 5 (Adversarial Review → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 29 | `monitor-store.test.ts` has existing `paneMode` test (line 43-44) and `describe('setPaneMode')` block (lines 187-209) that will fail after Task 1 removes those features | Blocker | Added to Task 1 Step 3: remove the paneMode assertion test and the entire setPaneMode describe block (3 tests) |
| 30 | After `git mv TerminalPane.tsx` to `terminal/` subdir, internal import `../../hooks/use-terminal-socket` resolves to wrong path (`src/components/hooks/` instead of `src/hooks/`) | Blocker | Added to Task 8 Step 3: update import from `../../` to `../../../hooks/use-terminal-socket` with before/after code |
| 31 | `ExpandedPaneOverlay.tsx` Task 4 Step 8 lists interface + display removal but misses destructuring removal (line 81) | Warning | Added item 2 to Step 8: "Remove `mode` from props destructuring (line 81)" — now 3 explicit removals |
| 32 | Task 3 Step 3 (empty message filtering) had no code example and didn't clarify `tool_use` exception | Warning | Added explicit code example for the message path and DO NOT check list for tool_use and error types |
