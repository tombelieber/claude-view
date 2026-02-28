# Session Card: AskUserQuestion & Compacting UI — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add inline question preview and compacting overlay to Mission Control session cards.

**Architecture:** Two independent frontend components — `QuestionCard` (new) and compacting overlay (added to existing `ContextGauge`). Both consume data already present in `agentState.context` and `agentState.label`. No backend changes.

**Tech Stack:** React, TypeScript, Tailwind CSS, Lucide icons, Vitest + React Testing Library.

**Design doc:** `docs/plans/2026-02-20-session-card-question-compacting-design.md`

---

### Task 1: QuestionCard Component — Tests

**Files:**
- Create: `src/components/live/QuestionCard.test.tsx`

**Step 1: Write the tests**

```tsx
import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { QuestionCard } from './QuestionCard'

const singleQuestion = {
  questions: [
    {
      question: 'Which auth method should we use?',
      header: 'Auth',
      options: [
        { label: 'JWT tokens', description: 'Stateless auth with JSON web tokens' },
        { label: 'OAuth 2.0', description: 'Delegated auth via OAuth' },
        { label: 'Session cookies', description: 'Server-side session storage' },
      ],
      multiSelect: false,
    },
  ],
}

const multiQuestion = {
  questions: [
    { question: 'First question?', header: 'Q1', options: [{ label: 'A', description: 'a' }, { label: 'B', description: 'b' }], multiSelect: false },
    { question: 'Second question?', header: 'Q2', options: [{ label: 'C', description: 'c' }], multiSelect: false },
  ],
}

describe('QuestionCard', () => {
  it('renders nothing when context has no questions', () => {
    const { container } = render(<QuestionCard context={{}} />)
    expect(container.firstChild).toBeNull()
  })

  it('renders nothing when context is undefined', () => {
    const { container } = render(<QuestionCard context={undefined} />)
    expect(container.firstChild).toBeNull()
  })

  it('shows the question text', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText('Which auth method should we use?')).toBeInTheDocument()
  })

  it('shows option labels as read-only chips', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText('JWT tokens')).toBeInTheDocument()
    expect(screen.getByText('OAuth 2.0')).toBeInTheDocument()
    expect(screen.getByText('Session cookies')).toBeInTheDocument()
  })

  it('shows "Answer in terminal" footer', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.getByText(/answer in terminal/i)).toBeInTheDocument()
  })

  it('shows multi-question badge when questions.length > 1', () => {
    render(<QuestionCard context={multiQuestion} />)
    expect(screen.getByText(/1 of 2/)).toBeInTheDocument()
  })

  it('does not show multi-question badge for single question', () => {
    render(<QuestionCard context={singleQuestion} />)
    expect(screen.queryByText(/1 of/)).not.toBeInTheDocument()
  })

  it('limits displayed options to 4', () => {
    const manyOptions = {
      questions: [{
        question: 'Pick one?',
        header: 'Test',
        options: [
          { label: 'Opt1', description: '' },
          { label: 'Opt2', description: '' },
          { label: 'Opt3', description: '' },
          { label: 'Opt4', description: '' },
          { label: 'Opt5', description: '' },
        ],
        multiSelect: false,
      }],
    }
    render(<QuestionCard context={manyOptions} />)
    expect(screen.getByText('Opt1')).toBeInTheDocument()
    expect(screen.getByText('Opt4')).toBeInTheDocument()
    expect(screen.queryByText('Opt5')).not.toBeInTheDocument()
  })
})
```

**Step 2: Run tests to verify they fail**

Run: `bunx vitest run src/components/live/QuestionCard.test.tsx`
Expected: FAIL — `QuestionCard` module not found.

**Step 3: Commit**

```bash
git add src/components/live/QuestionCard.test.tsx
git commit -m "test: add QuestionCard component tests"
```

---

### Task 2: QuestionCard Component — Implementation

**Files:**
- Create: `src/components/live/QuestionCard.tsx`

**Step 1: Implement QuestionCard**

```tsx
import { MessageCircle, Keyboard } from 'lucide-react'

interface QuestionOption {
  label: string
  description: string
}

interface Question {
  question: string
  header: string
  options: QuestionOption[]
  multiSelect: boolean
}

interface QuestionCardProps {
  context?: Record<string, unknown>
}

export function QuestionCard({ context }: QuestionCardProps) {
  if (!context) return null

  const questions = context.questions as Question[] | undefined
  if (!questions || questions.length === 0) return null

  const q = questions[0]
  const displayOptions = q.options.slice(0, 4)
  const isMulti = questions.length > 1

  return (
    <div
      className="mb-2 rounded-md border-l-4 border-amber-400 dark:border-amber-500 bg-amber-50/50 dark:bg-amber-900/10 p-3"
      data-testid="question-card"
    >
      {/* Header */}
      <div className="flex items-center gap-1.5 mb-1.5">
        <MessageCircle className="h-3.5 w-3.5 text-amber-500 dark:text-amber-400" />
        <span className="text-[11px] font-semibold text-amber-600 dark:text-amber-400 uppercase tracking-wide">
          Question
        </span>
        {isMulti && (
          <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-200/60 dark:bg-amber-800/40 text-amber-700 dark:text-amber-300 font-medium">
            1 of {questions.length}
          </span>
        )}
      </div>

      {/* Question text */}
      <p className="text-sm font-medium text-gray-900 dark:text-gray-100 line-clamp-2 mb-2">
        {q.question}
      </p>

      {/* Options grid */}
      {displayOptions.length > 0 && (
        <div className="grid grid-cols-2 gap-1.5 mb-2">
          {displayOptions.map((opt) => (
            <span
              key={opt.label}
              className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs bg-white/80 dark:bg-gray-800/60 text-gray-600 dark:text-gray-400 border border-gray-200 dark:border-gray-700"
              title={opt.description}
            >
              <span className="w-2 h-2 rounded-full border border-gray-300 dark:border-gray-600 flex-shrink-0" />
              <span className="truncate">{opt.label}</span>
            </span>
          ))}
        </div>
      )}

      {/* Footer */}
      <div className="flex items-center gap-1 text-[10px] text-gray-400 dark:text-gray-500">
        <Keyboard className="h-2.5 w-2.5" />
        <span>Answer in terminal</span>
      </div>
    </div>
  )
}
```

**Step 2: Run tests to verify they pass**

Run: `bunx vitest run src/components/live/QuestionCard.test.tsx`
Expected: All 7 tests PASS.

**Step 3: Commit**

```bash
git add src/components/live/QuestionCard.tsx
git commit -m "feat: add QuestionCard component for inline AskUserQuestion display"
```

---

### Task 3: Wire QuestionCard into SessionCard

**Files:**
- Modify: `src/components/live/SessionCard.tsx:1-17` (imports) and `:148-153` (between spinner and task progress)

**Step 1: Add import**

At `SessionCard.tsx:13`, after the `TaskProgressList` import, add:

```tsx
import { QuestionCard } from './QuestionCard'
```

**Step 2: Add QuestionCard slot between spinner row and task progress**

At `SessionCard.tsx:148-150`, between the spinner `</div>` and the task progress section, insert:

```tsx
      {/* Question card (AskUserQuestion) */}
      {session.agentState.state === 'awaiting_input' && session.agentState.context && (
        <QuestionCard context={session.agentState.context} />
      )}
```

**Step 3: Run existing SessionCard tests to verify no regressions**

Run: `bunx vitest run src/components/live/SessionCard.test.tsx`
Expected: All existing tests PASS.

**Step 4: Commit**

```bash
git add src/components/live/SessionCard.tsx
git commit -m "feat: wire QuestionCard into SessionCard layout"
```

---

### Task 4: SessionCard QuestionCard Integration Test

**Files:**
- Modify: `src/components/live/SessionCard.test.tsx`

**Step 1: Add integration test**

Append to `SessionCard.test.tsx`:

```tsx
describe('SessionCard question card', () => {
  it('shows QuestionCard when state is awaiting_input with question context', () => {
    const session = createMockSession({
      agentState: {
        group: 'needs_you',
        state: 'awaiting_input',
        label: 'Asked you a question',
        context: {
          questions: [{
            question: 'Which database should we use?',
            header: 'DB',
            options: [
              { label: 'PostgreSQL', description: 'Relational' },
              { label: 'SQLite', description: 'Embedded' },
            ],
            multiSelect: false,
          }],
        },
      },
    })
    renderCard(session)
    expect(screen.getByTestId('question-card')).toBeInTheDocument()
    expect(screen.getByText('Which database should we use?')).toBeInTheDocument()
    expect(screen.getByText('PostgreSQL')).toBeInTheDocument()
  })

  it('does not show QuestionCard when awaiting_input but no context', () => {
    const session = createMockSession({
      agentState: {
        group: 'needs_you',
        state: 'awaiting_input',
        label: 'Waiting for your next message',
      },
    })
    renderCard(session)
    expect(screen.queryByTestId('question-card')).not.toBeInTheDocument()
  })

  it('does not show QuestionCard when autonomous state', () => {
    const session = createMockSession({
      agentState: {
        group: 'autonomous',
        state: 'acting',
        label: 'Using tools...',
      },
    })
    renderCard(session)
    expect(screen.queryByTestId('question-card')).not.toBeInTheDocument()
  })
})
```

**Step 2: Run all SessionCard tests**

Run: `bunx vitest run src/components/live/SessionCard.test.tsx`
Expected: All tests PASS (old + new).

**Step 3: Commit**

```bash
git add src/components/live/SessionCard.test.tsx
git commit -m "test: add QuestionCard integration tests in SessionCard"
```

---

### Task 5: ContextGauge Compacting Overlay — Tests

**Files:**
- Create: `src/components/live/ContextGauge.test.tsx`

**Step 1: Write the tests**

```tsx
import { describe, expect, it, vi, afterEach } from 'vitest'
import { render, screen, act } from '@testing-library/react'
import { ContextGauge } from './ContextGauge'

const baseProps = {
  contextWindowTokens: 80_000,
  model: 'claude-sonnet-4',
  group: 'autonomous' as const,
  tokens: { inputTokens: 80_000, outputTokens: 10_000, cacheReadTokens: 0, cacheCreationTokens: 0, totalTokens: 90_000 },
  turnCount: 20,
}

describe('ContextGauge compacting overlay', () => {
  afterEach(() => { vi.useRealTimers() })

  it('shows compacting label when agentLabel contains "compacting"', () => {
    render(<ContextGauge {...baseProps} agentLabel="Auto-compacting context..." />)
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })

  it('does not show compacting label during normal thinking', () => {
    render(<ContextGauge {...baseProps} agentLabel="Thinking..." />)
    expect(screen.queryByText(/compacting/i)).not.toBeInTheDocument()
  })

  it('shows compacted label briefly after compacting ends', () => {
    vi.useFakeTimers()
    const { rerender } = render(<ContextGauge {...baseProps} agentLabel="Compacting context..." />)

    // State transitions away from compacting
    rerender(<ContextGauge {...baseProps} agentLabel="Using tools..." />)
    expect(screen.getByText(/compacted/i)).toBeInTheDocument()

    // After 5 seconds, the label should disappear
    act(() => { vi.advanceTimersByTime(5_000) })
    expect(screen.queryByText(/compacted/i)).not.toBeInTheDocument()
  })

  it('does not show compacting in expanded mode either', () => {
    render(<ContextGauge {...baseProps} agentLabel="Auto-compacting context..." expanded />)
    expect(screen.getByText(/compacting/i)).toBeInTheDocument()
  })
})
```

**Step 2: Run tests to verify they fail**

Run: `bunx vitest run src/components/live/ContextGauge.test.tsx`
Expected: FAIL — `agentLabel` prop does not exist yet on `ContextGauge`.

**Step 3: Commit**

```bash
git add src/components/live/ContextGauge.test.tsx
git commit -m "test: add ContextGauge compacting overlay tests"
```

---

### Task 6: ContextGauge Compacting Overlay — Implementation

**Files:**
- Modify: `src/components/live/ContextGauge.tsx`

**Step 1: Add `agentLabel` prop to `ContextGaugeProps` interface**

At `ContextGauge.tsx:20-37`, add the new prop:

```tsx
interface ContextGaugeProps {
  contextWindowTokens: number
  model: string | null
  group: AgentStateGroup
  tokens?: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    totalTokens: number
  }
  turnCount?: number
  expanded?: boolean
  /** Current agent state label — used to detect compacting state. */
  agentLabel?: string
}
```

**Step 2: Add compacting detection logic**

At the top of the `ContextGauge` function (after the existing state declarations around line 62), add:

```tsx
import { useState, useRef, useCallback, useLayoutEffect, useEffect } from 'react'
import { Minimize2 } from 'lucide-react'

// ... inside ContextGauge function, after existing state declarations:

  // Compacting state detection
  const isCompacting = agentLabel ? /compacting/i.test(agentLabel) : false
  const prevLabelRef = useRef(agentLabel)
  const [justCompacted, setJustCompacted] = useState(false)

  useEffect(() => {
    const wasCompacting = prevLabelRef.current ? /compacting/i.test(prevLabelRef.current) : false
    if (wasCompacting && !isCompacting) {
      setJustCompacted(true)
      const timer = setTimeout(() => setJustCompacted(false), 5_000)
      return () => clearTimeout(timer)
    }
    prevLabelRef.current = agentLabel
  }, [agentLabel, isCompacting])
```

**Step 3: Add compacting label to compact mode (line ~241-248)**

Replace the existing info row below the gauge bar:

```tsx
      <div className="flex items-center justify-between text-[10px] text-gray-400 dark:text-gray-500">
        <span>{formatTokens(contextWindowTokens)}/{formatTokens(contextLimit)} tokens</span>
        <span className="flex items-center gap-1">
          {isCompacting && (
            <>
              <Minimize2 className="h-3 w-3 text-blue-500 dark:text-blue-400 motion-safe:animate-pulse" />
              <span className="text-blue-500 dark:text-blue-400">compacting...</span>
            </>
          )}
          {!isCompacting && justCompacted && (
            <span className="text-green-500 dark:text-green-400 animate-pulse">
              compacted
            </span>
          )}
          {!isCompacting && !justCompacted && usedPct > 75 && (
            <span className={usedPct > 90 ? 'text-red-500' : 'text-amber-500'} title="Context is filling up. Auto-compaction may occur soon.">
              {usedPct.toFixed(0)}% used
            </span>
          )}
        </span>
      </div>
```

Also add `motion-safe:animate-pulse` to the gauge bar when compacting:

```tsx
        {usedPct > 0 && (
          <div
            className={`${barColor} h-full transition-all duration-300 ${isCompacting ? 'motion-safe:animate-pulse' : ''}`}
            style={{ width: `${usedPct}%` }}
          />
        )}
```

**Step 4: Run tests to verify they pass**

Run: `bunx vitest run src/components/live/ContextGauge.test.tsx`
Expected: All 4 tests PASS.

**Step 5: Commit**

```bash
git add src/components/live/ContextGauge.tsx
git commit -m "feat: add compacting overlay to ContextGauge"
```

---

### Task 7: Wire agentLabel into ContextGauge from SessionCard

**Files:**
- Modify: `src/components/live/SessionCard.tsx:163-169`

**Step 1: Pass `agentLabel` prop**

Update the `ContextGauge` invocation in `SessionCard.tsx`:

```tsx
      <ContextGauge
        contextWindowTokens={session.contextWindowTokens}
        model={session.model}
        group={session.agentState.group}
        tokens={session.tokens}
        turnCount={session.turnCount}
        agentLabel={session.agentState.label}
      />
```

**Step 2: Run all live component tests**

Run: `bunx vitest run src/components/live/`
Expected: All tests PASS.

**Step 3: Commit**

```bash
git add src/components/live/SessionCard.tsx
git commit -m "feat: wire agentLabel through SessionCard to ContextGauge"
```

---

### Task 8: Visual Verification

**Step 1: Build and check for type errors**

Run: `bun run build`
Expected: Build succeeds with no TypeScript errors.

**Step 2: Run all tests**

Run: `bunx vitest run`
Expected: All tests PASS.

**Step 3: Manual visual check**

1. Run `bun run dev`
2. Open browser to Mission Control live view
3. Verify session cards render correctly
4. If a session has an active `AskUserQuestion` event, verify the inline question card appears with amber left border, question text, and option chips
5. If no live session has these states, verify no visual regressions on normal cards

**Step 4: Commit (if any build fixes needed)**

```bash
git add -A
git commit -m "fix: address build/type issues from question card and compacting overlay"
```

---

### Task 9: Final Commit — Squash-Ready Summary

All changes across this plan:

| File | Status | Purpose |
|------|--------|---------|
| `src/components/live/QuestionCard.tsx` | NEW | Inline question preview component |
| `src/components/live/QuestionCard.test.tsx` | NEW | Unit tests for QuestionCard |
| `src/components/live/SessionCard.tsx` | MODIFIED | Wire QuestionCard + pass agentLabel |
| `src/components/live/SessionCard.test.tsx` | MODIFIED | Integration tests for QuestionCard in SessionCard |
| `src/components/live/ContextGauge.tsx` | MODIFIED | Compacting overlay with post-compact fade |
| `src/components/live/ContextGauge.test.tsx` | NEW | Tests for compacting overlay |
