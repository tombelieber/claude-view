import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { SidecarConnection } from '../../../hooks/use-sidecar-connection'

// --- Mocks ---

const mockConnection: SidecarConnection = {
  isLive: true,
  status: 'active',
  committedBlocks: [
    { id: 'b1', type: 'user', text: 'Hello', timestamp: 1710000000 },
    { id: 'b2', type: 'assistant', text: 'Hi there', timestamp: 1710000001 },
  ] as SidecarConnection['committedBlocks'],
  pendingText: '',
  model: 'claude-sonnet-4-20250514',
  permissionMode: 'default',
  contextTokens: 12000,
  contextLimit: 200000,
  contextPercent: 6,
  totalCost: 0.15,
  send: vi.fn(),
  disconnect: vi.fn(),
}

let mockUseSidecarConnection = vi.fn(
  (_sessionId: string, _opts?: { skip?: boolean }) => mockConnection,
)

vi.mock('../../../hooks/use-sidecar-connection', () => ({
  useSidecarConnection: (sessionId: string, opts?: { skip?: boolean }) =>
    mockUseSidecarConnection(sessionId, opts),
}))

vi.mock('../../conversation/ConversationThread', () => ({
  ConversationThread: ({ blocks }: { blocks: unknown[] }) => (
    <div data-testid="conversation-thread">{blocks.length} blocks</div>
  ),
}))

vi.mock('../ChatPanelHeader', () => ({
  ChatPanelHeader: () => <div data-testid="chat-panel-header" />,
}))

vi.mock('../ChatStatusBar', () => ({
  ChatStatusBar: ({
    model,
    contextTokens,
    totalCost,
  }: { model: string; contextTokens: number; totalCost: number | null }) => (
    <div
      data-testid="chat-status-bar"
      data-model={model}
      data-tokens={contextTokens}
      data-cost={totalCost}
    />
  ),
}))

vi.mock('../ChatInputBar', () => ({
  ChatInputBar: () => <div data-testid="chat-input-bar" />,
}))

import { ChatPanel } from '../ChatPanel'

function renderPanel(overrides?: { sessionId?: string; isWatching?: boolean }) {
  const params = {
    sessionId: overrides?.sessionId ?? 'sess-123',
    isWatching: overrides?.isWatching ?? false,
  }
  // Simulate dockview panel props shape
  const props = {
    params,
    api: {} as unknown,
    containerApi: {} as unknown,
  }
  // biome-ignore lint/suspicious/noExplicitAny: mock dockview props in test
  return render(<ChatPanel {...(props as any)} />)
}

describe('ChatPanel', () => {
  beforeEach(() => {
    mockUseSidecarConnection = vi.fn(() => mockConnection)
  })

  it('renders ConversationThread with live blocks when connected', () => {
    renderPanel()
    expect(screen.getByTestId('conversation-thread')).toBeDefined()
    expect(screen.getByTestId('conversation-thread').textContent).toBe('2 blocks')
  })

  it('renders ConversationThread with history blocks when disconnected', () => {
    mockUseSidecarConnection = vi.fn(() => ({
      ...mockConnection,
      isLive: false,
      status: 'ended' as const,
      committedBlocks: [],
    }))
    renderPanel()
    expect(screen.getByTestId('conversation-thread')).toBeDefined()
    expect(screen.getByTestId('conversation-thread').textContent).toBe('0 blocks')
  })

  it('renders ChatStatusBar with connection data', () => {
    renderPanel()
    const bar = screen.getByTestId('chat-status-bar')
    expect(bar.getAttribute('data-model')).toBe('claude-sonnet-4-20250514')
    expect(bar.getAttribute('data-tokens')).toBe('12000')
    expect(bar.getAttribute('data-cost')).toBe('0.15')
  })

  it('renders ChatInputBar', () => {
    renderPanel()
    expect(screen.getByTestId('chat-input-bar')).toBeDefined()
  })

  it('passes skip=true to useSidecarConnection when isWatching', () => {
    renderPanel({ isWatching: true })
    expect(mockUseSidecarConnection).toHaveBeenCalledWith('sess-123', { skip: true })
  })
})
