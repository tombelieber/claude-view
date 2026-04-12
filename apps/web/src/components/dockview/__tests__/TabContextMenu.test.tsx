import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Mock session mutations
vi.mock('../../../hooks/use-session-mutations', () => ({
  useSessionMutations: () => ({ deleteSession: { mutate: vi.fn() } }),
}))

const { TabContextMenu } = await import('../TabContextMenu')

function createMockPanel(overrides: Record<string, unknown> = {}) {
  return {
    id: 'panel-1',
    title: 'Test Panel',
    params: { sessionId: 'session-1' },
    api: {
      close: vi.fn(),
      maximize: vi.fn(),
      isMaximized: vi.fn(() => false),
      exitMaximized: vi.fn(),
    },
    ...overrides,
  }
}

function createMockApi(overrides: Record<string, unknown> = {}) {
  const panel = createMockPanel()
  return {
    panels: [panel],
    addPanel: vi.fn(),
    hasMaximizedGroup: vi.fn(() => false),
    exitMaximizedGroup: vi.fn(),
    ...overrides,
  }
}

async function openContextMenu(triggerText: string) {
  const trigger = screen.getByText(triggerText)
  fireEvent.contextMenu(trigger)
  // Wait for Radix portal to render
  await vi.waitFor(() => {
    expect(screen.getByText('Close')).toBeInTheDocument()
  })
}

describe('TabContextMenu', () => {
  describe('existing menu items', () => {
    it('renders Close, Close Others, Close All', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')

      expect(screen.getByText('Close')).toBeInTheDocument()
      expect(screen.getByText('Close Others')).toBeInTheDocument()
      expect(screen.getByText('Close All')).toBeInTheDocument()
    })

    it('renders Split Right and Split Down', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')

      expect(screen.getByText('Split Right')).toBeInTheDocument()
      expect(screen.getByText('Split Down')).toBeInTheDocument()
    })

    it('renders Copy Session ID', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')

      expect(screen.getByText('Copy Session ID')).toBeInTheDocument()
    })
  })

  describe('new menu items — zoom', () => {
    it('renders Zoom Pane item', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      expect(screen.getByText('Zoom Pane')).toBeInTheDocument()
    })

    it('calls panel.api.maximize() when Zoom Pane is clicked and not maximized', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      fireEvent.click(screen.getByText('Zoom Pane'))
      expect(panel.api.maximize).toHaveBeenCalledTimes(1)
    })

    it('shows Exit Zoom when panel is maximized', async () => {
      const panel = createMockPanel()
      panel.api.isMaximized = vi.fn(() => true)
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      expect(screen.getByText('Exit Zoom')).toBeInTheDocument()
    })

    it('calls exitMaximized when Exit Zoom is clicked', async () => {
      const panel = createMockPanel()
      panel.api.isMaximized = vi.fn(() => true)
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      fireEvent.click(screen.getByText('Exit Zoom'))
      expect(panel.api.exitMaximized).toHaveBeenCalledTimes(1)
    })
  })

  describe('new menu items — session metadata', () => {
    it('renders session metadata section when session info is in params', async () => {
      const panel = createMockPanel({
        params: {
          sessionId: 'session-1',
          projectName: 'acme/my-project',
          branch: 'feat/live-sessions',
          cost: '$0.42',
          contextPct: '67%',
          turn: 14,
          activity: 'Editing handler',
        },
      })
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      expect(screen.getByText('acme/my-project')).toBeInTheDocument()
      expect(screen.getByText(/feat\/live-sessions/)).toBeInTheDocument()
    })
  })

  describe('shortcut labels', () => {
    it('shows Ctrl+Shift+Enter next to Zoom Pane', async () => {
      const panel = createMockPanel()
      const api = createMockApi()
      render(
        <TabContextMenu panel={panel as never} api={api as never} splitComponent="session">
          <div>Tab Trigger</div>
        </TabContextMenu>,
      )

      await openContextMenu('Tab Trigger')
      // Shortcut text should be present in the same menu item row
      const zoomItem = screen.getByText('Zoom Pane').closest('[class]')
      expect(zoomItem?.textContent).toContain('Ctrl+Shift+Enter')
    })
  })
})
