import { render, screen, fireEvent } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SessionInteractionCard } from '../SessionInteractionCard'
import { CompactInteractionPreview } from '../CompactInteractionPreview'
import { InteractionError } from '../InteractionError'
import type { PendingInteractionMeta, FullInteractionBlock } from '../SessionInteractionCard'

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

function makeMeta(
  overrides: Partial<PendingInteractionMeta> = {},
): PendingInteractionMeta {
  return {
    variant: 'permission',
    requestId: 'req-1',
    preview: 'Wants to run echo hello',
    ...overrides,
  }
}

function makeFullPermission(
  overrides: Partial<FullInteractionBlock> = {},
): FullInteractionBlock {
  return {
    id: 'ib-1',
    variant: 'permission',
    requestId: 'req-1',
    resolved: false,
    historicalSource: null,
    data: {
      type: 'permission_request',
      requestId: 'req-1',
      toolName: 'Bash',
      toolInput: { command: 'echo hello' },
      toolUseID: 'tu-1',
      timeoutMs: 60_000,
    },
    ...overrides,
  }
}

function makeFullQuestion(
  overrides: Partial<FullInteractionBlock> = {},
): FullInteractionBlock {
  return {
    id: 'ib-2',
    variant: 'question',
    requestId: 'req-2',
    resolved: false,
    historicalSource: null,
    data: {
      type: 'ask_question',
      requestId: 'req-2',
      questions: [
        {
          question: 'Continue?',
          header: 'Confirm',
          options: [
            { label: 'Yes', description: 'Proceed' },
            { label: 'No', description: 'Cancel' },
          ],
          multiSelect: false,
        },
      ],
    },
    ...overrides,
  }
}

function makeFullPlan(
  overrides: Partial<FullInteractionBlock> = {},
): FullInteractionBlock {
  return {
    id: 'ib-3',
    variant: 'plan',
    requestId: 'req-3',
    resolved: false,
    historicalSource: null,
    data: {
      type: 'plan_approval',
      requestId: 'req-3',
      planData: { plan: 'Step 1: do thing' },
    },
    ...overrides,
  }
}

function makeFullElicitation(
  overrides: Partial<FullInteractionBlock> = {},
): FullInteractionBlock {
  return {
    id: 'ib-4',
    variant: 'elicitation',
    requestId: 'req-4',
    resolved: false,
    historicalSource: null,
    data: {
      type: 'elicitation',
      requestId: 'req-4',
      toolName: 'custom-tool',
      toolInput: {},
      prompt: 'Enter value:',
    },
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// InteractionError
// ---------------------------------------------------------------------------

describe('InteractionError', () => {
  it('renders the variant name in the error message', () => {
    render(<InteractionError variant="permission" />)
    expect(
      screen.getByText(/could not display permission interaction/i),
    ).toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// CompactInteractionPreview
// ---------------------------------------------------------------------------

describe('CompactInteractionPreview', () => {
  it('renders variant label and preview text', () => {
    const meta = makeMeta({ variant: 'permission', preview: 'run echo hello' })
    render(<CompactInteractionPreview meta={meta} />)

    expect(screen.getByText('Permission')).toBeInTheDocument()
    expect(screen.getByText('run echo hello')).toBeInTheDocument()
  })

  it('renders correct label for each variant', () => {
    const variants = [
      { variant: 'permission' as const, label: 'Permission' },
      { variant: 'question' as const, label: 'Question' },
      { variant: 'plan' as const, label: 'Plan' },
      { variant: 'elicitation' as const, label: 'Input' },
    ]

    for (const { variant, label } of variants) {
      const { unmount } = render(
        <CompactInteractionPreview meta={makeMeta({ variant })} />,
      )
      expect(screen.getByText(label)).toBeInTheDocument()
      unmount()
    }
  })

  it('falls back to variant string for unknown variant', () => {
    const meta = { variant: 'unknown_type' as any, requestId: 'r-1', preview: 'test' }
    render(<CompactInteractionPreview meta={meta} />)
    expect(screen.getByText('unknown_type')).toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// SessionInteractionCard
// ---------------------------------------------------------------------------

describe('SessionInteractionCard', () => {
  it('renders CompactInteractionPreview when fullInteraction is null', () => {
    const meta = makeMeta({ preview: 'Loading preview...' })
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={meta}
        fullInteraction={null}
      />,
    )

    // Should show the compact preview, not a card
    expect(screen.getByText('Permission')).toBeInTheDocument()
    expect(screen.getByText('Loading preview...')).toBeInTheDocument()
    // Should NOT show the permission tool name since full data isn't loaded
    expect(screen.queryByText('Bash')).not.toBeInTheDocument()
  })

  it('renders PermissionCard for permission variant with valid data', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta()}
        fullInteraction={makeFullPermission()}
        respond={vi.fn()}
      />,
    )

    // PermissionCard renders tool name
    expect(screen.getByText('Bash')).toBeInTheDocument()
    // Interactive buttons present
    expect(screen.getByRole('button', { name: /allow/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /deny/i })).toBeInTheDocument()
  })

  it('renders InteractionError when normalizer returns null (invalid data)', () => {
    const invalid = makeFullPermission({
      data: { broken: true } as any, // missing requestId and toolName
    })

    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta()}
        fullInteraction={invalid}
      />,
    )

    expect(
      screen.getByText(/could not display permission interaction/i),
    ).toBeInTheDocument()
  })

  it('omits action buttons when respond is undefined (read-only)', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta()}
        fullInteraction={makeFullPermission()}
        // no respond prop => read-only
      />,
    )

    // Card content renders
    expect(screen.getByText('Bash')).toBeInTheDocument()
    // But no action buttons
    expect(
      screen.queryByRole('button', { name: /allow/i }),
    ).not.toBeInTheDocument()
    expect(
      screen.queryByRole('button', { name: /deny/i }),
    ).not.toBeInTheDocument()
  })

  it('renders AskUserQuestionCard for question variant', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta({ variant: 'question' })}
        fullInteraction={makeFullQuestion()}
        respond={vi.fn()}
      />,
    )

    expect(screen.getByText('Continue?')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /submit/i })).toBeInTheDocument()
  })

  it('renders PlanApprovalCard for plan variant', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta({ variant: 'plan' })}
        fullInteraction={makeFullPlan()}
        respond={vi.fn()}
      />,
    )

    expect(screen.getByText(/step 1/i)).toBeInTheDocument()
  })

  it('renders ElicitationCard for elicitation variant', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta({ variant: 'elicitation' })}
        fullInteraction={makeFullElicitation()}
        respond={vi.fn()}
      />,
    )

    expect(screen.getByText(/enter value/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /submit/i })).toBeInTheDocument()
  })

  it('wires onAlwaysAllow for permission cards with suggestions', () => {
    const respond = vi.fn()
    const suggestion = { type: 'allow_tool', toolName: 'Bash' }
    const full = makeFullPermission({
      data: {
        type: 'permission_request',
        requestId: 'req-1',
        toolName: 'Bash',
        toolInput: { command: 'echo hello' },
        toolUseID: 'tu-1',
        timeoutMs: 60_000,
        suggestions: [suggestion],
      },
    })

    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta()}
        fullInteraction={full}
        respond={respond}
      />,
    )

    expect(
      screen.getByRole('button', { name: /always allow/i }),
    ).toBeInTheDocument()
  })

  it('does NOT show Always Allow when permission has no suggestions', () => {
    render(
      <SessionInteractionCard
        sessionId="s-1"
        meta={makeMeta()}
        fullInteraction={makeFullPermission()}
        respond={vi.fn()}
      />,
    )

    expect(
      screen.queryByRole('button', { name: /always allow/i }),
    ).not.toBeInTheDocument()
  })
})
