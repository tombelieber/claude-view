import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useNavigate: () => vi.fn(),
}))

// Mock ChatSession — ChatPanel is just a thin wrapper
vi.mock('../../../pages/ChatSession', () => ({
  ChatSession: ({ sessionId, liveStatus }: { sessionId?: string; liveStatus?: string }) => (
    <div
      data-testid="chat-session"
      data-session-id={sessionId ?? ''}
      data-live-status={liveStatus ?? 'inactive'}
    />
  ),
}))

import { ChatPanel } from '../ChatPanel'

function renderPanel(overrides?: { sessionId?: string; liveStatus?: string }) {
  const params = {
    sessionId: overrides?.sessionId ?? 'sess-123',
    liveStatus: overrides?.liveStatus ?? 'inactive',
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

  it('passes liveStatus to ChatSession', () => {
    renderPanel({ liveStatus: 'cc_owned' })
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-live-status')).toBe('cc_owned')
  })

  it('passes liveStatus=inactive by default', () => {
    renderPanel()
    const el = screen.getByTestId('chat-session')
    expect(el.getAttribute('data-live-status')).toBe('inactive')
  })
})
