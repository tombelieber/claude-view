import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

// Capture what SessionPanel renders by intercepting DockviewReact
let capturedComponents: Record<string, React.ComponentType<{ params: Record<string, unknown> }>>
let capturedOnReady: ((event: { api: unknown }) => void) | undefined

vi.mock('dockview-react', () => ({
  DockviewReact: ({
    components,
    onReady,
  }: {
    components: Record<string, React.ComponentType<{ params: Record<string, unknown> }>>
    onReady: (event: { api: unknown }) => void
    [key: string]: unknown
  }) => {
    capturedComponents = components
    capturedOnReady = onReady
    return <div data-testid="dockview-mock" />
  },
}))

// Mock CliTerminal to check if it's rendered directly
vi.mock('../../cli-terminal/CliTerminal', () => ({
  CliTerminal: ({ tmuxSessionId, embedded }: { tmuxSessionId: string; embedded?: boolean }) => (
    <div data-testid="cli-terminal" data-tmux-id={tmuxSessionId} data-embedded={embedded} />
  ),
}))

// Mock BlockTerminalPane
vi.mock('../BlockTerminalPane', () => ({
  BlockTerminalPane: ({ sessionId }: { sessionId: string }) => (
    <div data-testid="block-terminal" data-session-id={sessionId} />
  ),
}))

// Mock MonitorPane to detect if it's used
vi.mock('../MonitorPane', () => ({
  MonitorPane: ({ children, embedded }: { children: React.ReactNode; embedded?: boolean }) => (
    <div data-testid="monitor-pane" data-embedded={embedded}>
      {children}
    </div>
  ),
}))

// Mock monitor store
vi.mock('../../../store/monitor-store', () => ({
  useMonitorStore: Object.assign(
    (selector: (s: Record<string, unknown>) => unknown) =>
      selector({
        compactHeaders: false,
        selectedPaneId: null,
        pinnedPaneIds: new Set(),
        selectPane: vi.fn(),
        pinPane: vi.fn(),
        unpinPane: vi.fn(),
        hidePane: vi.fn(),
      }),
    { getState: () => ({}) },
  ),
}))

// Mock persistence hook
vi.mock('../../../hooks/use-dockview-persistence', () => ({
  useDockviewPersistence: vi.fn(() => vi.fn()),
}))

const { DockLayout, DockPaneContext } = await import('../DockLayout')

function createSession(id: string, hasTmux = true) {
  return {
    id,
    slug: `session-${id}`,
    projectDisplayName: `project-${id}`,
    status: 'working',
    agentState: { group: 'autonomous', state: 'tool_use', label: 'Working', context: null },
    ownership: hasTmux ? { tmux: { cliSessionId: `tmux-${id}` } } : undefined,
    subAgents: [],
    turnCount: 5,
  }
}

describe('DockLayout', () => {
  describe('SessionPanel renders CliTerminal directly without MonitorPane', () => {
    it('renders CliTerminal with embedded=true for tmux sessions', () => {
      const sessions = [createSession('s1')]
      render(
        <DockLayout
          sessions={sessions as never}
          initialLayout={null}
          onLayoutChange={vi.fn()}
          compactHeaders={false}
          displayMode="chat"
        />,
      )

      const SessionPanel = capturedComponents.session
      expect(SessionPanel).toBeDefined()

      // Render SessionPanel wrapped in the same context DockLayout uses
      const { getByTestId, queryByTestId } = render(
        <DockPaneContext.Provider value={{ sessions: sessions as never }}>
          <SessionPanel params={{ sessionId: 's1', displayMode: 'chat', status: 'working' }} />
        </DockPaneContext.Provider>,
      )

      const terminal = getByTestId('cli-terminal')
      expect(terminal).toBeInTheDocument()
      expect(terminal.dataset.embedded).toBe('true')
      expect(terminal.dataset.tmuxId).toBe('tmux-s1')
      expect(queryByTestId('monitor-pane')).not.toBeInTheDocument()
    })

    it('renders BlockTerminalPane for non-tmux sessions', () => {
      const sessions = [createSession('s2', false)]
      render(
        <DockLayout
          sessions={sessions as never}
          initialLayout={null}
          onLayoutChange={vi.fn()}
          compactHeaders={false}
          displayMode="chat"
        />,
      )

      const SessionPanel = capturedComponents.session
      const { getByTestId, queryByTestId } = render(
        <DockPaneContext.Provider value={{ sessions: sessions as never }}>
          <SessionPanel params={{ sessionId: 's2', displayMode: 'chat', status: 'working' }} />
        </DockPaneContext.Provider>,
      )

      expect(getByTestId('block-terminal')).toBeInTheDocument()
      expect(queryByTestId('cli-terminal')).not.toBeInTheDocument()
      expect(queryByTestId('monitor-pane')).not.toBeInTheDocument()
    })

    it('shows "Session ended" fallback when session not found in context', () => {
      render(
        <DockLayout
          sessions={[] as never}
          initialLayout={null}
          onLayoutChange={vi.fn()}
          compactHeaders={false}
          displayMode="chat"
        />,
      )

      const SessionPanel = capturedComponents.session
      const { getByText } = render(
        <DockPaneContext.Provider value={{ sessions: [] }}>
          <SessionPanel
            params={{ sessionId: 'nonexistent', displayMode: 'chat', status: 'done' }}
          />
        </DockPaneContext.Provider>,
      )

      expect(getByText('Session ended')).toBeInTheDocument()
    })
  })

  describe('addPanel uses minimumWidth and minimumHeight', () => {
    it('sets minimum constraints on panels created in onReady', () => {
      const addPanel = vi.fn()
      const mockApi = {
        panels: [],
        addPanel,
        removePanel: vi.fn(),
        fromJSON: vi.fn(),
        clear: vi.fn(),
        toJSON: vi.fn(),
        onDidAddPanel: { addListener: vi.fn(() => ({ dispose: vi.fn() })) },
        onDidRemovePanel: { addListener: vi.fn(() => ({ dispose: vi.fn() })) },
        onDidLayoutChange: { addListener: vi.fn(() => ({ dispose: vi.fn() })) },
        onDidActivePanelChange: { addListener: vi.fn(() => ({ dispose: vi.fn() })) },
      }

      render(
        <DockLayout
          sessions={[createSession('s1')] as never}
          initialLayout={null}
          onLayoutChange={vi.fn()}
          compactHeaders={false}
          displayMode="chat"
        />,
      )

      // Fire onReady
      capturedOnReady?.({ api: mockApi })

      expect(addPanel).toHaveBeenCalled()
      const call = addPanel.mock.calls[0][0]
      expect(call.minimumWidth).toBe(400)
      expect(call.minimumHeight).toBe(200)
    })
  })
})
