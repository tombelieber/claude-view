---
status: done
date: 2026-02-02
---

# Hardening Final: 2 Remaining Fixes

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the last 2 gaps from the original 7-fix hardening plan. Everything else is already implemented and tested (287 tests green).

**Architecture:** Wire the existing ErrorBoundary component into the Virtuoso message list so a single broken message can't crash the whole conversation view. Add a `console.warn` when thread nesting exceeds the max depth cap.

**Tech Stack:** React 19, Vitest, happy-dom, existing ErrorBoundary class component

**Supersedes:** `2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` (5/7 fixes were already done)

---

## Status of Original 7 Fixes

| # | Fix | Status |
|---|-----|--------|
| 1 | DOMPurify + StructuredDataCard | **DONE** — 9 tests in `StructuredDataCard.test.tsx` |
| 2 | UntrustedData XSS | **DONE** — tests in `UntrustedData.test.tsx` |
| 3 | AgentProgressCard XSS | **DONE** — tests in `AgentProgressCard.test.tsx` |
| 4 | ErrorBoundary integration | **PARTIAL** — component + 12 tests exist, NOT wired into ConversationView |
| 5 | Max nesting depth warning | **PARTIAL** — caps at 5 levels, no `console.warn()` |
| 6 | Null/undefined handling (10 components) | **DONE** — tests in `NullSafety.test.tsx` |
| 7 | useEffect cleanup | **DONE** — 20+ tests in `UseEffectCleanup.test.tsx` |

---

## Task 1: Wire ErrorBoundary into ConversationView

**Files:**
- Modify: `src/components/ConversationView.tsx:1-8` (add import)
- Modify: `src/components/ConversationView.tsx:245-256` (wrap MessageTyped)
- Test: `src/components/ErrorBoundary.test.tsx` (add integration test)

**Step 1: Write the failing test**

Add to the bottom of `src/components/ErrorBoundary.test.tsx`, inside a new describe block:

```tsx
describe('Integration: ErrorBoundary wraps messages', () => {
  it('should catch a crashing MessageTyped without killing the list', () => {
    // Simulate: first child crashes, second child survives
    const Crasher = () => { throw new Error('boom') }

    const { container } = render(
      <div>
        <ErrorBoundary>
          <Crasher />
        </ErrorBoundary>
        <ErrorBoundary>
          <div data-testid="survivor">I survived</div>
        </ErrorBoundary>
      </div>
    )

    // Crashed message shows boundary, sibling still renders
    expect(screen.getByTestId('error-boundary')).toBeInTheDocument()
    expect(screen.getByTestId('survivor')).toBeInTheDocument()
  })
})
```

**Step 2: Run test to verify it passes**

This test should already pass because ErrorBoundary works — it's a confidence test for the integration pattern.

Run: `bun run test:client -- --run ErrorBoundary`
Expected: All tests PASS (existing 12 + 1 new)

**Step 3: Add ErrorBoundary import to ConversationView**

In `src/components/ConversationView.tsx`, add the import alongside `MessageTyped`:

```tsx
import { ErrorBoundary } from './ErrorBoundary'
```

**Step 4: Wrap MessageTyped in ErrorBoundary inside Virtuoso**

In `src/components/ConversationView.tsx`, replace the `itemContent` callback (lines 245-256):

Old:
```tsx
itemContent={(index, message) => (
  <div className="max-w-4xl mx-auto px-6 pb-4">
    <MessageTyped
      key={message.uuid || index}
      message={message}
      messageIndex={index}
      messageType={message.role}
      metadata={message.metadata}
      parentUuid={message.parent_uuid ?? undefined}
    />
  </div>
)}
```

New:
```tsx
itemContent={(index, message) => (
  <div className="max-w-4xl mx-auto px-6 pb-4">
    <ErrorBoundary key={message.uuid || index}>
      <MessageTyped
        message={message}
        messageIndex={index}
        messageType={message.role}
        metadata={message.metadata}
        parentUuid={message.parent_uuid ?? undefined}
      />
    </ErrorBoundary>
  </div>
)}
```

Note: Move `key` from `MessageTyped` to `ErrorBoundary` — the outermost element in the list needs the key. This also means a key change resets the error state (correct behavior: re-mounting clears a caught error).

**Step 5: Run full test suite**

Run: `bun run test:client -- --run`
Expected: 22 test files pass, 288+ tests green

**Step 6: Commit**

```bash
git add src/components/ConversationView.tsx src/components/ErrorBoundary.test.tsx
git commit -m "fix(ui): wrap messages in ErrorBoundary so one crash doesn't kill the view"
```

---

## Task 2: Add console.warn for max nesting overflow

**Files:**
- Modify: `src/components/MessageTyped.tsx:266-268` (add warn)
- Test: `src/components/MessageTyped.test.tsx` (add nesting warn test)

**Step 1: Write the failing test**

Add to `src/components/MessageTyped.test.tsx`:

```tsx
describe('Nesting depth warning', () => {
  it('should console.warn when indent exceeds MAX_INDENT_LEVEL', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

    render(
      <MessageTyped
        message={{ uuid: '1', role: 'user', content: 'deep', timestamp: null, thinking: null, tool_calls: [], parent_uuid: null, metadata: null }}
        indent={10}
      />
    )

    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining('Max nesting depth')
    )
    warnSpy.mockRestore()
  })

  it('should NOT warn when indent is within MAX_INDENT_LEVEL', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})

    render(
      <MessageTyped
        message={{ uuid: '2', role: 'user', content: 'shallow', timestamp: null, thinking: null, tool_calls: [], parent_uuid: null, metadata: null }}
        indent={3}
      />
    )

    expect(warnSpy).not.toHaveBeenCalled()
    warnSpy.mockRestore()
  })
})
```

**Step 2: Run test to verify it fails**

Run: `bun run test:client -- --run MessageTyped`
Expected: FAIL — `console.warn` is never called

**Step 3: Add the warning**

In `src/components/MessageTyped.tsx`, after line 267 (`const clampedIndent = ...`), add:

```tsx
if (indent > MAX_INDENT_LEVEL) {
  console.warn(`Max nesting depth (${MAX_INDENT_LEVEL}) exceeded: indent=${indent}, clamped to ${clampedIndent}`)
}
```

**Step 4: Run test to verify it passes**

Run: `bun run test:client -- --run MessageTyped`
Expected: PASS — both new tests green

**Step 5: Run full test suite**

Run: `bun run test:client -- --run`
Expected: 22 test files pass, 290+ tests green

**Step 6: Commit**

```bash
git add src/components/MessageTyped.tsx src/components/MessageTyped.test.tsx
git commit -m "fix(ui): warn when thread nesting exceeds max depth"
```

---

## Task 3: Mark hardening as done in PROGRESS.md

**Files:**
- Modify: `docs/plans/PROGRESS.md`
- Modify: `docs/plans/2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` (frontmatter only)

**Step 1: Update PROGRESS.md**

In the Plan File Index table, change the hardening row status from `pending` to `done`:

```
| `2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md` | done | **Completed** — 7 TDD-first fixes (DOMPurify, XSS, error boundaries, nesting, null safety) |
```

**Step 2: Update old plan frontmatter**

If the old plan has YAML frontmatter, change `status: pending` to `status: done`.

**Step 3: Commit**

```bash
git add docs/plans/PROGRESS.md docs/plans/2026-01-29-HARDENING-IMPLEMENTATION-PLAN-V2-FINAL.md
git commit -m "docs: mark hardening plan as done"
```

---

## Exit Criteria

```
All 7 original fixes verified:
[x] #1 DOMPurify + StructuredDataCard (already done)
[x] #2 UntrustedData XSS (already done)
[x] #3 AgentProgressCard XSS (already done)
[ ] #4 ErrorBoundary wired into ConversationView  <-- Task 1
[ ] #5 console.warn on nesting overflow            <-- Task 2
[x] #6 Null/undefined handling (already done)
[x] #7 useEffect cleanup (already done)

Test suite: 22 files pass, 290+ tests green
TypeScript: bun run typecheck — 0 errors
```
