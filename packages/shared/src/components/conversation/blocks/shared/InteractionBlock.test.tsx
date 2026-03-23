import type { InteractionBlock as InteractionBlockType } from '../../../../types/blocks'
import type { PermissionRequest } from '../../../../types/sidecar-protocol'
import { fireEvent, render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { ConversationActionsProvider } from '../../../../contexts/conversation-actions-context'
import type { ConversationActions } from '../../../../contexts/conversation-actions-context'
import { ChatInteractionBlock } from '../chat/InteractionBlock'
import { DevInteractionBlock } from '../developer/InteractionBlock'
import { _clearRespondedCacheForTesting } from './use-interaction-handlers'

beforeEach(() => {
  _clearRespondedCacheForTesting()
})

// ---------------------------------------------------------------------------
// Factories
// ---------------------------------------------------------------------------

function makePermissionBlock(overrides: Partial<PermissionRequest> = {}): InteractionBlockType {
  return {
    id: 'ib-1',
    type: 'interaction',
    variant: 'permission',
    requestId: 'req-1',
    resolved: false,
    data: {
      type: 'permission_request',
      requestId: 'req-1',
      toolName: 'Bash',
      toolInput: { command: 'echo hello' },
      toolUseID: 'tu-1',
      timeoutMs: 60_000,
      ...overrides,
    } satisfies PermissionRequest,
  }
}

function makeQuestionBlock(): InteractionBlockType {
  return {
    id: 'ib-2',
    type: 'interaction',
    variant: 'question',
    requestId: 'req-2',
    resolved: false,
    data: {
      type: 'ask_question',
      requestId: 'req-2',
      questions: [
        {
          question: 'Continue?',
          header: 'Confirm',
          options: [
            { label: 'Yes', description: 'Proceed with the action' },
            { label: 'No', description: 'Cancel the action' },
          ],
          multiSelect: false,
        },
      ],
    },
  }
}

function makePlanBlock(): InteractionBlockType {
  return {
    id: 'ib-3',
    type: 'interaction',
    variant: 'plan',
    requestId: 'req-3',
    resolved: false,
    data: {
      type: 'plan_approval',
      requestId: 'req-3',
      planData: { plan: 'Step 1: do thing' },
    },
  }
}

function makeElicitationBlock(): InteractionBlockType {
  return {
    id: 'ib-4',
    type: 'interaction',
    variant: 'elicitation',
    requestId: 'req-4',
    resolved: false,
    data: {
      type: 'elicitation',
      requestId: 'req-4',
      toolName: 'custom-tool',
      toolInput: {},
      prompt: 'Enter value:',
    },
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeActions(overrides: Partial<ConversationActions> = {}): ConversationActions {
  return {
    retryMessage: vi.fn(),
    ...overrides,
  }
}

function renderWithActions(ui: React.ReactNode, overrides: Partial<ConversationActions> = {}) {
  const actions = makeActions(overrides)
  return render(<ConversationActionsProvider actions={actions}>{ui}</ConversationActionsProvider>)
}

// ---------------------------------------------------------------------------
// Tests: PermissionCard wiring via context
// ---------------------------------------------------------------------------

describe('InteractionBlock — Permission card via context', () => {
  it('renders Allow/Deny buttons when respondPermission is in context', () => {
    const respondPermission = vi.fn()
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, { respondPermission })

    expect(screen.getByRole('button', { name: /allow/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /deny/i })).toBeInTheDocument()
  })

  it('does NOT render Allow/Deny buttons without context handlers', () => {
    render(<ChatInteractionBlock block={makePermissionBlock()} />)

    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
  })

  it('clicking Allow calls respondPermission with (requestId, true)', () => {
    const respondPermission = vi.fn()
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, { respondPermission })

    fireEvent.click(screen.getByRole('button', { name: /allow/i }))
    expect(respondPermission).toHaveBeenCalledWith('req-1', true)
  })

  it('clicking Deny calls respondPermission with (requestId, false)', () => {
    const respondPermission = vi.fn()
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, { respondPermission })

    fireEvent.click(screen.getByRole('button', { name: /deny/i }))
    expect(respondPermission).toHaveBeenCalledWith('req-1', false)
  })

  it('server-resolved permission card hides buttons', () => {
    const block = makePermissionBlock()
    block.resolved = true
    renderWithActions(<ChatInteractionBlock block={block} />, { respondPermission: vi.fn() })

    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
  })

  it('developer variant also wires from context', () => {
    const respondPermission = vi.fn()
    renderWithActions(<DevInteractionBlock block={makePermissionBlock()} />, { respondPermission })

    expect(screen.getByRole('button', { name: /allow/i })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /allow/i }))
    expect(respondPermission).toHaveBeenCalledWith('req-1', true)
  })
})

// ---------------------------------------------------------------------------
// Tests: Local resolution — buttons disappear after click
// ---------------------------------------------------------------------------

describe('InteractionBlock — local resolution after user responds', () => {
  it('clicking Allow hides buttons and shows Allowed badge', () => {
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, {
      respondPermission: vi.fn(),
    })

    // Buttons present before click
    expect(screen.getByRole('button', { name: /allow/i })).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /allow/i }))

    // Buttons gone after click
    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
    // Badge shown
    expect(screen.getByText(/allowed/i)).toBeInTheDocument()
  })

  it('clicking Deny hides buttons and shows Denied badge', () => {
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, {
      respondPermission: vi.fn(),
    })

    fireEvent.click(screen.getByRole('button', { name: /deny/i }))

    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
    expect(screen.getByText(/denied/i)).toBeInTheDocument()
  })

  it('double-click prevention: handler only called once', () => {
    const respondPermission = vi.fn()
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, { respondPermission })

    const allowBtn = screen.getByRole('button', { name: /allow/i })
    fireEvent.click(allowBtn)
    // After first click, buttons are gone — can't click again
    expect(respondPermission).toHaveBeenCalledTimes(1)
    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
  })

  it('plan card: clicking Approve shows Approved badge', () => {
    renderWithActions(<ChatInteractionBlock block={makePlanBlock()} />, { approvePlan: vi.fn() })

    fireEvent.click(screen.getByRole('button', { name: /approve/i }))

    expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument()
    expect(screen.getByText(/approved/i)).toBeInTheDocument()
  })

  it('developer variant also resolves locally', () => {
    renderWithActions(<DevInteractionBlock block={makePermissionBlock()} />, {
      respondPermission: vi.fn(),
    })

    fireEvent.click(screen.getByRole('button', { name: /allow/i }))

    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
    expect(screen.getByText(/allowed/i)).toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// Tests: All 4 interactive types render through context
// ---------------------------------------------------------------------------

describe('InteractionBlock — all 4 variants wire from context', () => {
  it('permission block renders tool name badge', () => {
    renderWithActions(<ChatInteractionBlock block={makePermissionBlock()} />, {
      respondPermission: vi.fn(),
    })
    expect(screen.getByText('Bash')).toBeInTheDocument()
  })

  it('question block renders question text and submit button', () => {
    renderWithActions(<ChatInteractionBlock block={makeQuestionBlock()} />, {
      answerQuestion: vi.fn(),
    })
    expect(screen.getByText('Continue?')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /submit/i })).toBeInTheDocument()
  })

  it('plan block renders plan content and approve button', () => {
    renderWithActions(<ChatInteractionBlock block={makePlanBlock()} />, { approvePlan: vi.fn() })
    expect(screen.getByText(/step 1/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /approve/i })).toBeInTheDocument()
  })

  it('elicitation block renders prompt and submit button', () => {
    renderWithActions(<ChatInteractionBlock block={makeElicitationBlock()} />, {
      submitElicitation: vi.fn(),
    })
    expect(screen.getByText(/enter value/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /submit/i })).toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// Tests: Without context, cards render read-only (no buttons)
// ---------------------------------------------------------------------------

describe('InteractionBlock — read-only without context', () => {
  it('permission card renders content but no buttons', () => {
    render(<ChatInteractionBlock block={makePermissionBlock()} />)
    expect(screen.getByText('Bash')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
  })

  it('question card renders content but no submit button', () => {
    render(<ChatInteractionBlock block={makeQuestionBlock()} />)
    expect(screen.getByText('Continue?')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /submit/i })).not.toBeInTheDocument()
  })

  it('plan card renders content but no approve button', () => {
    render(<ChatInteractionBlock block={makePlanBlock()} />)
    expect(screen.getByText(/step 1/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /approve/i })).not.toBeInTheDocument()
  })

  it('elicitation card renders prompt but no submit button', () => {
    render(<ChatInteractionBlock block={makeElicitationBlock()} />)
    expect(screen.getByText(/enter value/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /submit/i })).not.toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// Tests: Mode switch persistence — resolved state survives unmount/remount
// ---------------------------------------------------------------------------

describe('InteractionBlock — mode switch persistence', () => {
  it('Allow response persists when switching Chat → Developer (same requestId)', () => {
    const block = makePermissionBlock()
    const actions = makeActions({ respondPermission: vi.fn() })

    // Render in Chat mode, click Allow
    const { unmount } = render(
      <ConversationActionsProvider actions={actions}>
        <ChatInteractionBlock block={block} />
      </ConversationActionsProvider>,
    )
    fireEvent.click(screen.getByRole('button', { name: /allow/i }))
    expect(screen.getByText(/allowed/i)).toBeInTheDocument()

    // Unmount (simulates mode switch)
    unmount()

    // Remount in Developer mode with SAME block (same requestId)
    render(
      <ConversationActionsProvider actions={actions}>
        <DevInteractionBlock block={block} />
      </ConversationActionsProvider>,
    )

    // Should still show Allowed, no buttons
    expect(screen.getByText(/allowed/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /allow/i })).not.toBeInTheDocument()
  })

  it('Deny response persists when switching Chat → Developer', () => {
    const block = makePermissionBlock({ requestId: 'persist-deny-test' })
    block.requestId = 'persist-deny-test'
    const actions = makeActions({ respondPermission: vi.fn() })

    const { unmount } = render(
      <ConversationActionsProvider actions={actions}>
        <ChatInteractionBlock block={block} />
      </ConversationActionsProvider>,
    )
    fireEvent.click(screen.getByRole('button', { name: /deny/i }))
    expect(screen.getByText(/denied/i)).toBeInTheDocument()
    unmount()

    render(
      <ConversationActionsProvider actions={actions}>
        <DevInteractionBlock block={block} />
      </ConversationActionsProvider>,
    )
    expect(screen.getByText(/denied/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deny/i })).not.toBeInTheDocument()
  })

  it('different requestIds are independent', () => {
    const block1 = makePermissionBlock({ requestId: 'independent-1' })
    block1.requestId = 'independent-1'
    const block2 = makePermissionBlock({ requestId: 'independent-2' })
    block2.requestId = 'independent-2'
    const actions = makeActions({ respondPermission: vi.fn() })

    // Respond to block1
    const { unmount } = render(
      <ConversationActionsProvider actions={actions}>
        <ChatInteractionBlock block={block1} />
      </ConversationActionsProvider>,
    )
    fireEvent.click(screen.getByRole('button', { name: /allow/i }))
    unmount()

    // block2 should still have buttons (different requestId)
    render(
      <ConversationActionsProvider actions={actions}>
        <ChatInteractionBlock block={block2} />
      </ConversationActionsProvider>,
    )
    expect(screen.getByRole('button', { name: /allow/i })).toBeInTheDocument()
  })
})
