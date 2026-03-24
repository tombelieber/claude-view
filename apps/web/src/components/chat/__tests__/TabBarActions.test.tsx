// Unit tests for TabBarActions — guards split panel behavior.
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { IDockviewHeaderActionsProps } from 'dockview-core'
import { describe, expect, it, vi } from 'vitest'

// Minimal mock that satisfies IDockviewHeaderActionsProps for render
function makeMockProps(overrides?: Partial<IDockviewHeaderActionsProps>) {
  return {
    containerApi: { addPanel: vi.fn(), panels: [] },
    activePanel: {
      id: 'panel-1',
      title: 'Test Panel',
      params: { sessionId: 'sess-123' },
    },
    api: {},
    panels: [],
    isGroupActive: true,
    group: {},
    headerPosition: 'top',
    ...overrides,
    // biome-ignore lint/suspicious/noExplicitAny: test stub — mocking dockview internals
  } as any as IDockviewHeaderActionsProps
}

// Must mock radix before import
vi.mock('@radix-ui/react-dropdown-menu', async () => {
  const actual = await vi.importActual('@radix-ui/react-dropdown-menu')
  return actual
})

import { TabBarActions } from '../TabBarActions'

describe('TabBarActions', () => {
  it('renders the split menu trigger button', () => {
    render(<TabBarActions {...makeMockProps()} />)
    expect(screen.getByRole('button')).toBeInTheDocument()
  })

  it('split right creates panel with title "New Chat" and empty sessionId', async () => {
    const user = userEvent.setup()
    const props = makeMockProps()

    render(<TabBarActions {...props} />)

    // Open dropdown
    await user.click(screen.getByRole('button'))
    // Click Split Right
    const splitRight = await screen.findByText('Split Right')
    await user.click(splitRight)

    // biome-ignore lint/suspicious/noExplicitAny: extract mock from dockview stub
    const addPanel = (props.containerApi as any).addPanel as ReturnType<typeof vi.fn>
    expect(addPanel).toHaveBeenCalledTimes(1)
    const args = addPanel.mock.calls[0][0]

    // Panel ID should start with 'chat-new-' (not 'chat-' without 'new')
    expect(args.id).toMatch(/^chat-new-\d+$/)
    // Title must be "New Chat", not a timestamp
    expect(args.title).toBe('New Chat')
    // Params should have empty sessionId (blank panel)
    expect(args.params).toEqual({ sessionId: '' })
    // Position should reference the active panel, direction right
    expect(args.position.direction).toBe('right')
  })

  it('split down creates panel with direction below', async () => {
    const user = userEvent.setup()
    const props = makeMockProps()

    render(<TabBarActions {...props} />)

    await user.click(screen.getByRole('button'))
    const splitDown = await screen.findByText('Split Down')
    await user.click(splitDown)

    // biome-ignore lint/suspicious/noExplicitAny: extract mock from dockview stub
    const addPanel = (props.containerApi as any).addPanel as ReturnType<typeof vi.fn>
    expect(addPanel).toHaveBeenCalledTimes(1)
    const args = addPanel.mock.calls[0][0]
    expect(args.position.direction).toBe('below')
  })

  it('does nothing when activePanel is null', () => {
    render(<TabBarActions {...makeMockProps({ activePanel: undefined })} />)
    // Should render without crashing
    expect(screen.getByRole('button')).toBeInTheDocument()
  })
})
