# Claude View Fixes Design

**Date:** 2026-01-26
**Status:** Approved

## Overview

Two UX fixes for the claude-view session browser:

1. **Project name parsing** — Use filesystem verification to correctly extract project names from Claude's encoded directory names
2. **SPA navigation** — Keep sidebar visible when viewing conversations

## Fix 1: Filesystem-Verified Project Names

### Problem

Claude encodes project paths using hyphens as separators:
- `/Users/TBGor/dev/@vicky-ai/claude-view` → `-Users-TBGor-dev--vicky-ai-claude-view`

The current decoder can't distinguish path separators from literal hyphens in directory names. A project named `claude-view` might incorrectly decode as `claude/view`.

### Solution

After decoding, verify the path exists on the filesystem:

```
Encoded: -Users-TBGor-dev--vicky-ai-claude-view
Decoded: /Users/TBGor/dev/@vicky-ai/claude-view
Verify:  fs.existsSync('/Users/TBGor/dev/@vicky-ai/claude-view') → true
Extract: basename → "claude-view"
```

### Implementation

**File:** `src/server/sessions.ts`

1. Add `resolveProjectPath(encodedName: string)` function:
   - Decode using current logic (remove leading `-`, replace `--` with `@`, replace `-` with `/`)
   - Prepend `/` to make absolute path
   - Check if path exists with `fs.existsSync()`
   - If exists, return `{ fullPath, projectName: basename(fullPath) }`
   - If not exists, fall back to encoded name as display

2. Update `getProjects()` to use resolved paths for display names

### Result

Sidebar shows `claude-view` instead of incorrectly decoded paths.

---

## Fix 2: Persistent Sidebar (True SPA)

### Problem

When clicking a session, the entire view switches — sidebar disappears and is replaced by `ConversationView`. User expects sidebar to remain visible.

### Current Behavior

```tsx
// App.tsx
{selectedSession ? (
  <ConversationView ... />  // Replaces everything
) : (
  <>
    <Sidebar ... />
    <MainContent ... />
  </>
)}
```

### Solution

Move sidebar outside the conditional:

```tsx
<Sidebar ... />  // Always visible
{selectedSession ? (
  <ConversationView ... />  // Only replaces main content
) : (
  <MainContent ... />
)}
```

### Implementation

**File:** `src/App.tsx`

1. Restructure JSX to keep `<Sidebar>` always rendered
2. `ConversationView` replaces only `MainContent` area
3. "Back" button clears `selectedSession` state
4. Sidebar highlights the project containing the active session

### Result

Clicking a session swaps only the main panel; sidebar stays visible for navigation.

---

## Files Changed

| File | Change |
|------|--------|
| `src/server/sessions.ts` | Add `resolveProjectPath()`, update display name logic |
| `src/App.tsx` | Restructure layout to keep sidebar persistent |

## Scope

- 2 files
- No new dependencies
- Backward compatible
