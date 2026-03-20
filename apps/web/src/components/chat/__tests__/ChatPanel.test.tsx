import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}))

// Mock ChatSession — ChatPanel is just a thin wrapper
vi.mock('../../../pages/ChatSession', () => ({
  ChatSession: ({ sessionId, isWatching }: { sessionId?: string; isWatching?: boolean }) => (
    <div
      data-testid="chat-session"
      data-session-id={sessionId ?? ''}
      data-watching={isWatching ? 'true' : 'false'}
    />
  ),
}))

import { ChatPanel } from '../ChatPanel'

function renderPanel(overrides?: { sessionId?: string; isWatching?: boolean }) {
  const params = {
    sessionId: overrides?.sessionId ?? 'sess-123',
    isWatching: overrides?.isWatching ?? false,
  }
  const props = {
    params,
    api: {} as unknown,
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

  it('passes isWatching to ChatSession', () => {
    renderPanel({ isWatching: true })
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-watching')).toBe('true')
  })

  it('passes isWatching=false by default', () => {
    renderPanel()
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-watching')).toBe('false')
  })
})
