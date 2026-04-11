import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}))

// Mock ChatSession — ChatPanel is just a thin wrapper
vi.mock('../../../pages/ChatSession', () => ({
  ChatSession: ({
    sessionId,
    ownershipTier,
  }: { sessionId?: string; ownershipTier?: string | null }) => (
    <div
      data-testid="chat-session"
      data-session-id={sessionId ?? ''}
      data-ownership-tier={ownershipTier ?? ''}
    />
  ),
}))

// Mock CliTerminal
vi.mock('../../cli-terminal/CliTerminal', () => ({
  CliTerminal: ({ tmuxSessionId }: { tmuxSessionId: string }) => (
    <div data-testid="cli-terminal" data-tmux-session-id={tmuxSessionId} />
  ),
}))

import { ChatPanel } from '../ChatPanel'

function renderPanel(overrides?: {
  sessionId?: string
  ownershipTier?: string | null
  tmuxSessionId?: string
}) {
  const params = {
    sessionId: overrides?.sessionId ?? 'sess-123',
    ownershipTier: overrides?.ownershipTier ?? null,
    tmuxSessionId: overrides?.tmuxSessionId,
  }
  const props = {
    params,
    api: {
      isActive: false,
      onDidActiveChange: vi.fn(() => ({ dispose: vi.fn() })),
      onDidGroupChange: vi.fn(() => ({ dispose: vi.fn() })),
      updateParameters: vi.fn(),
      setTitle: vi.fn(),
    } as unknown,
    containerApi: {} as unknown,
  }
  // biome-ignore lint/suspicious/noExplicitAny: mock dockview props in test
  return render(<ChatPanel {...(props as any)} />)
}

describe('ChatPanel', () => {
  it('renders ChatSession with sessionId', () => {
    renderPanel({ sessionId: 'abc-123' })
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-session-id')).toBe('abc-123')
  })

  it('passes ownershipTier to ChatSession', () => {
    renderPanel({ ownershipTier: 'tmux' })
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-ownership-tier')).toBe('tmux')
  })

  it('passes ownershipTier=null by default', () => {
    renderPanel()
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-ownership-tier')).toBe('')
  })

  it('renders CliTerminal when ownershipTier is tmux and tmuxSessionId is set', () => {
    renderPanel({ ownershipTier: 'tmux', tmuxSessionId: 'cv-abc123' })
    expect(screen.getByTestId('cli-terminal')).toBeInTheDocument()
    expect(screen.getByTestId('cli-terminal').getAttribute('data-tmux-session-id')).toBe(
      'cv-abc123',
    )
    expect(screen.queryByTestId('chat-session')).not.toBeInTheDocument()
  })

  it('renders ChatSession when ownershipTier is tmux but tmuxSessionId is missing', () => {
    // Edge case: ownership resolved but tmuxSessionId not yet in params
    renderPanel({ ownershipTier: 'tmux' })
    expect(screen.getByTestId('chat-session')).toBeInTheDocument()
    expect(screen.queryByTestId('cli-terminal')).not.toBeInTheDocument()
  })

  it('renders ChatSession for sdk ownership', () => {
    renderPanel({ ownershipTier: 'sdk' })
    expect(screen.getByTestId('chat-session')).toBeInTheDocument()
    expect(screen.queryByTestId('cli-terminal')).not.toBeInTheDocument()
  })

  it('panel container has min-w-0 and overflow-hidden for responsive shrinking', () => {
    renderPanel()
    const session = screen.getByTestId('chat-session')
    const container = session.parentElement
    expect(container).not.toBeNull()
    // Without min-w-0, the dockview panel content won't shrink when browser narrows.
    // overflow-hidden clips content and creates a new block formatting context.
    expect(container?.className).toContain('min-w-0')
    expect(container?.className).toContain('overflow-hidden')
  })
})
