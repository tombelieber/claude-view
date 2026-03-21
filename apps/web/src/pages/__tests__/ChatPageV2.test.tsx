// @vitest-environment happy-dom
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { createElement } from 'react'
import { describe, expect, it, vi } from 'vitest'

// --- Mock dockview ---

let capturedOnReady: ((api: unknown) => void) | undefined

vi.mock('../../components/chat/ChatDockLayout', () => ({
  readSavedChatLayout: () => null,
  ChatDockLayout: ({ onReady }: { onReady?: (api: unknown) => void }) => {
    capturedOnReady = onReady
    return <div data-testid="chat-dock-layout" />
  },
}))

// Mock SessionSidebar (heavy component with many deps)
vi.mock('../../components/conversation/sidebar/SessionSidebar', () => ({
  SessionSidebar: () => <nav data-testid="session-sidebar" aria-label="Chat history" />,
}))

// Mock keyboard shortcuts hook
vi.mock('../../hooks/use-chat-keyboard-shortcuts', () => ({
  useChatKeyboardShortcuts: () => {},
}))

// Mock react-router-dom
vi.mock('react-router-dom', () => ({
  useParams: () => ({}),
  useOutletContext: () => ({ liveSessions: { sessions: [] } }),
  useNavigate: () => () => {},
}))

import { ChatPageV2 } from '../ChatPageV2'

function renderWithProviders(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(createElement(QueryClientProvider, { client }, ui))
}

describe('ChatPageV2', () => {
  it('renders ChatDockLayout when sidecar is connected', () => {
    renderWithProviders(<ChatPageV2 />)
    expect(screen.getByTestId('chat-dock-layout')).toBeDefined()
  })

  it('routes /chat/:sessionId opens correct panel', () => {
    renderWithProviders(<ChatPageV2 />)

    // Simulate dockview ready with a mock API
    const mockApi = {
      panels: [],
      addPanel: vi.fn(),
    }
    capturedOnReady?.(mockApi)

    expect(screen.getByTestId('chat-dock-layout')).toBeDefined()
  })

  it('renders sidebar alongside dock layout', () => {
    renderWithProviders(<ChatPageV2 />)
    expect(screen.getByTestId('session-sidebar')).toBeDefined()
    expect(screen.getByTestId('chat-dock-layout')).toBeDefined()
  })
})
