import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import type { SessionOwnership } from '@claude-view/shared/types/generated/SessionOwnership'

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}))

// Mock ChatSession — ChatPanel is just a thin wrapper
vi.mock('../../../pages/ChatSession', () => ({
  ChatSession: ({
    sessionId,
    ownership,
  }: { sessionId?: string; ownership?: SessionOwnership | null }) => (
    <div
      data-testid="chat-session"
      data-session-id={sessionId ?? ''}
      data-ownership={ownership ? JSON.stringify(ownership) : ''}
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
  ownership?: SessionOwnership | null
  tmuxSessionId?: string
}) {
  const params = {
    sessionId: overrides?.sessionId ?? 'sess-123',
    ownership: overrides?.ownership ?? null,
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

  it('passes ownership to ChatSession', () => {
    renderPanel({ ownership: { tmux: { cliSessionId: 'cv-1' } } })
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-ownership')).toBe(
      JSON.stringify({ tmux: { cliSessionId: 'cv-1' } }),
    )
  })

  it('passes ownership=null by default', () => {
    renderPanel()
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-ownership')).toBe('')
  })

  it('renders CliTerminal when tmuxSessionId is set', () => {
    renderPanel({ tmuxSessionId: 'cv-abc123' })
    expect(screen.getByTestId('cli-terminal')).toBeInTheDocument()
    expect(screen.getByTestId('cli-terminal').getAttribute('data-tmux-session-id')).toBe(
      'cv-abc123',
    )
    expect(screen.queryByTestId('chat-session')).not.toBeInTheDocument()
  })

  it('renders ChatSession when tmuxSessionId is missing', () => {
    // Edge case: ownership resolved but tmuxSessionId not yet in params
    renderPanel({ ownership: { tmux: { cliSessionId: 'cv-1' } } })
    expect(screen.getByTestId('chat-session')).toBeInTheDocument()
    expect(screen.queryByTestId('cli-terminal')).not.toBeInTheDocument()
  })

  it('renders ChatSession for sdk ownership', () => {
    renderPanel({ ownership: { sdk: { controlId: 'ctl-1' } } })
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
