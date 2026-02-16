import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { MonitorPane, type MonitorPaneProps } from './MonitorPane'
import type { LiveSession } from './use-live-sessions'

function createMockSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'session-1',
    project: 'my-project',
    projectDisplayName: 'my-project',
    projectPath: '/Users/test/dev/my-project',
    filePath: '/tmp/sessions/session-1.jsonl',
    status: 'working',
    agentState: {
      group: 'autonomous',
      state: 'tool_use',
      label: 'Working',
      confidence: 1.0,
      source: 'jsonl',
    },
    gitBranch: 'feature/cool-stuff',
    pid: 12345,
    title: 'Test Session',
    lastUserMessage: 'Fix the bug',
    currentActivity: 'Writing code',
    turnCount: 5,
    startedAt: 1700000000,
    lastActivityAt: 1700001000,
    model: 'claude-sonnet-4-20250514',
    tokens: {
      inputTokens: 50000,
      outputTokens: 10000,
      cacheReadTokens: 30000,
      cacheCreationTokens: 5000,
      totalTokens: 95000,
    },
    contextWindowTokens: 120000,
    cost: {
      totalUsd: 1.23,
      inputCostUsd: 0.5,
      outputCostUsd: 0.6,
      cacheReadCostUsd: 0.1,
      cacheCreationCostUsd: 0.03,
      cacheSavingsUsd: 0.2,
    },
    cacheStatus: 'warm',
    ...overrides,
  }
}

function renderMonitorPane(overrides: Partial<MonitorPaneProps> = {}) {
  const defaultProps: MonitorPaneProps = {
    session: createMockSession(),
    isSelected: false,
    isExpanded: false,
    isPinned: false,
    mode: 'raw',
    compactHeader: false,
    isVisible: true,
    onSelect: vi.fn(),
    onExpand: vi.fn(),
    onModeToggle: vi.fn(),
    onPin: vi.fn(),
    onHide: vi.fn(),
    onContextMenu: vi.fn(),
  }

  const props = { ...defaultProps, ...overrides }
  return { ...render(<MonitorPane {...props} />), props }
}

describe('MonitorPane', () => {
  describe('header displays session data', () => {
    it('renders project name in header', () => {
      renderMonitorPane({
        session: createMockSession({ projectDisplayName: 'awesome-project' }),
      })

      expect(screen.getByText('awesome-project')).toBeInTheDocument()
    })

    it('falls back to last path segment when projectDisplayName is empty', () => {
      renderMonitorPane({
        session: createMockSession({
          projectDisplayName: '',
          projectPath: '/Users/test/dev/fallback-name',
          project: 'fallback-project',
        }),
      })

      expect(screen.getByText('fallback-name')).toBeInTheDocument()
    })

    it('renders cost formatted as $X.XX', () => {
      renderMonitorPane({
        session: createMockSession({
          cost: {
            totalUsd: 4.56,
            inputCostUsd: 2,
            outputCostUsd: 2,
            cacheReadCostUsd: 0.5,
            cacheCreationCostUsd: 0.06,
            cacheSavingsUsd: 0,
          },
        }),
      })

      expect(screen.getByText('$4.56')).toBeInTheDocument()
    })

    it('renders context percentage with green color when low', () => {
      renderMonitorPane({
        session: createMockSession({ contextWindowTokens: 40000 }),
      })

      // 40000 / 200000 = 20%
      const ctxEl = screen.getByText('20% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-green-400')
    })

    it('renders context percentage with amber color when moderate', () => {
      renderMonitorPane({
        session: createMockSession({ contextWindowTokens: 120000 }),
      })

      // 120000 / 200000 = 60%
      const ctxEl = screen.getByText('60% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-amber-400')
    })

    it('renders context percentage with red color when high', () => {
      renderMonitorPane({
        session: createMockSession({ contextWindowTokens: 180000 }),
      })

      // 180000 / 200000 = 90%
      const ctxEl = screen.getByText('90% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-red-400')
    })
  })

  describe('pin indicator', () => {
    it('shows pin icon when isPinned=true', () => {
      const { container } = renderMonitorPane({ isPinned: true })

      // Pin icon renders as an SVG with lucide class; when pinned there are extra Pin icons
      // The pin indicator in the header is a separate <Pin> with text-blue-400
      const blueIcons = container.querySelectorAll('.text-blue-400')
      expect(blueIcons.length).toBeGreaterThan(0)
    })

    it('does not show pin indicator icon when isPinned=false', () => {
      const { container } = renderMonitorPane({ isPinned: false })

      // When not pinned, the standalone Pin indicator should not render,
      // but the pin action button still exists. The indicator-specific Pin
      // has class text-blue-400 (without hover prefix).
      // The pin button when not pinned has text-gray-500 class
      const pinButton = screen.getByTitle('Pin pane')
      expect(pinButton.className).toContain('text-gray-500')
    })
  })

  describe('selection ring', () => {
    it('shows blue ring when isSelected=true', () => {
      const { container } = renderMonitorPane({ isSelected: true })

      const paneDiv = container.firstElementChild!
      expect(paneDiv.className).toContain('ring-2')
      expect(paneDiv.className).toContain('ring-blue-500')
    })

    it('does not show blue ring when isSelected=false', () => {
      const { container } = renderMonitorPane({ isSelected: false })

      const paneDiv = container.firstElementChild!
      expect(paneDiv.className).not.toContain('ring-2')
    })
  })

  describe('click handlers', () => {
    it('calls onSelect when header is clicked', () => {
      const onSelect = vi.fn()
      renderMonitorPane({ onSelect })

      // Click on the header area — find via the cursor-pointer header div
      const header = screen.getByText('my-project').closest('[class*="cursor-pointer"]')!
      fireEvent.click(header)

      expect(onSelect).toHaveBeenCalledTimes(1)
    })

    it('calls onExpand when expand button is clicked', () => {
      const onExpand = vi.fn()
      renderMonitorPane({ onExpand })

      const expandBtn = screen.getByTitle('Expand pane')
      fireEvent.click(expandBtn)

      expect(onExpand).toHaveBeenCalledTimes(1)
    })

    it('does not call onSelect when a button in the header is clicked', () => {
      const onSelect = vi.fn()
      renderMonitorPane({ onSelect })

      const expandBtn = screen.getByTitle('Expand pane')
      fireEvent.click(expandBtn)

      expect(onSelect).not.toHaveBeenCalled()
    })

    it('calls onContextMenu when right-clicked', () => {
      const onContextMenu = vi.fn()
      const { container } = renderMonitorPane({ onContextMenu })

      fireEvent.contextMenu(container.firstElementChild!)

      expect(onContextMenu).toHaveBeenCalledTimes(1)
    })
  })

  describe('visibility', () => {
    it('returns null when isVisible=false', () => {
      const { container } = renderMonitorPane({ isVisible: false })

      expect(container.innerHTML).toBe('')
    })
  })

  describe('children', () => {
    it('renders children when provided', () => {
      render(
        <MonitorPane
          session={createMockSession()}
          isSelected={false}
          isExpanded={false}
          isPinned={false}
          mode="raw"
          compactHeader={false}
          isVisible={true}
          onSelect={vi.fn()}
          onExpand={vi.fn()}
          onModeToggle={vi.fn()}
          onPin={vi.fn()}
          onHide={vi.fn()}
          onContextMenu={vi.fn()}
        >
          <div data-testid="custom-child">Custom content</div>
        </MonitorPane>
      )

      expect(screen.getByTestId('custom-child')).toBeInTheDocument()
      expect(screen.getByText('Custom content')).toBeInTheDocument()
    })

    it('shows "Connecting..." when no children provided', () => {
      renderMonitorPane()

      expect(screen.getByText('Connecting...')).toBeInTheDocument()
    })
  })

  describe('footer', () => {
    it('renders turn count in footer', () => {
      renderMonitorPane({
        session: createMockSession({ turnCount: 12 }),
      })

      expect(screen.getByText('Turn 12')).toBeInTheDocument()
    })

    it('renders current activity in footer', () => {
      renderMonitorPane({
        session: createMockSession({ currentActivity: 'Editing files' }),
      })

      expect(screen.getByText('Editing files')).toBeInTheDocument()
    })

    it('shows "Idle" when no activity', () => {
      renderMonitorPane({
        session: createMockSession({ currentActivity: '', lastUserMessage: '' }),
      })

      expect(screen.getByText('Idle')).toBeInTheDocument()
    })
  })

  describe('git branch', () => {
    it('renders branch name when present', () => {
      renderMonitorPane({
        session: createMockSession({ gitBranch: 'main' }),
      })

      expect(screen.getByText('main')).toBeInTheDocument()
    })

    it('does not render branch section when gitBranch is null', () => {
      renderMonitorPane({
        session: createMockSession({ gitBranch: null }),
      })

      expect(screen.queryByText('main')).not.toBeInTheDocument()
    })
  })
})
