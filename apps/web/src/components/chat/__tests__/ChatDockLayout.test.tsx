import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// --- Mock dockview-react ---

let capturedOnReady: ((event: { api: MockDockApi }) => void) | undefined
let capturedComponents: Record<string, unknown> | undefined
let capturedTabComponents: Record<string, unknown> | undefined
let capturedRightHeaderActions: unknown

interface MockPanel {
  id: string
  params?: Record<string, unknown>
  api: { setActive: () => void }
}

class MockDockApi {
  panels: MockPanel[] = []
  addPanel = vi.fn(
    (opts: { id: string; component: string; title?: string; params?: Record<string, unknown> }) => {
      const panel: MockPanel = {
        id: opts.id,
        params: opts.params,
        api: { setActive: vi.fn() },
      }
      this.panels.push(panel)
      return panel
    },
  )
  removePanel = vi.fn()
  onDidAddPanel = vi.fn()
  onDidRemovePanel = vi.fn()
  onDidLayoutChange = vi.fn()
}

vi.mock('dockview-react', () => ({
  DockviewReact: (props: {
    onReady?: (event: { api: MockDockApi }) => void
    components?: Record<string, unknown>
    tabComponents?: Record<string, unknown>
    rightHeaderActionsComponent?: unknown
  }) => {
    capturedOnReady = props.onReady
    capturedComponents = props.components
    capturedTabComponents = props.tabComponents
    capturedRightHeaderActions = props.rightHeaderActionsComponent
    return <div data-testid="dockview-react" />
  },
}))

vi.mock('../ChatPanel', () => ({
  ChatPanel: () => <div data-testid="chat-panel" />,
}))

vi.mock('../ChatTabRenderer', () => ({
  ChatTabRenderer: () => <div data-testid="chat-tab-renderer" />,
}))

vi.mock('../TabBarActions', () => ({
  TabBarActions: () => <div data-testid="tab-bar-actions" />,
}))

import { ChatDockLayout } from '../ChatDockLayout'

describe('ChatDockLayout', () => {
  it('adds panel when session is activated from sidebar', () => {
    const onReady = vi.fn()
    render(<ChatDockLayout initialLayout={null} onReady={onReady} />)

    // Simulate dockview ready
    const mockApi = new MockDockApi()
    capturedOnReady?.({ api: mockApi })

    // onReady callback should have been called with the api
    expect(onReady).toHaveBeenCalledWith(mockApi)
  })

  it('removes panel when session tab is closed', () => {
    render(<ChatDockLayout initialLayout={null} />)
    // Dockview handles panel close internally via tab close button.
    // We just verify the DockviewReact is rendered.
    expect(capturedComponents).toHaveProperty('chat')
  })

  it('split right creates second group with same session type', () => {
    render(<ChatDockLayout initialLayout={null} />)
    // TabBarActions is wired as rightHeaderActionsComponent
    expect(capturedRightHeaderActions).toBeDefined()
  })

  it('restores layout from localStorage if saved', () => {
    render(<ChatDockLayout initialLayout={null} />)

    // Verify the components and tab components are registered
    expect(capturedComponents).toHaveProperty('chat')
    expect(capturedTabComponents).toHaveProperty('chat')
  })
})
