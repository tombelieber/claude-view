import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { MonitorPane, type MonitorPaneProps } from './MonitorPane'
import type { LiveSession } from './use-live-sessions'

function createMockSession(overrides: Partial<LiveSession> = {}): LiveSession {
  const gitBranch = overrides.gitBranch !== undefined ? overrides.gitBranch : 'feature/cool-stuff'
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
      context: null,
    },
    gitBranch,
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: gitBranch,
    pid: 12345,
    title: 'Test Session',
    lastUserMessage: 'Fix the bug',
    currentActivity: 'Writing code',
    turnCount: 5,
    startedAt: 1700000000,
    lastActivityAt: 1700001000,
    model: 'claude-sonnet-4-20250514',
    currentTurnStartedAt: null,
    lastTurnTaskSeconds: null,
    tokens: {
      inputTokens: 50000,
      outputTokens: 10000,
      cacheReadTokens: 30000,
      cacheCreationTokens: 5000,
      cacheCreation5mTokens: 0,
      cacheCreation1hrTokens: 0,
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
      hasUnpricedUsage: false,
      unpricedInputTokens: 0,
      unpricedOutputTokens: 0,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 1,
      totalCostSource: 'computed_priced_tokens_full',
    },
    cacheStatus: 'warm',
    subAgents: [],
    teamName: null,
    progressItems: [],
    toolsUsed: [],
    lastCacheHitAt: null,
    compactCount: 0,
    slug: null,
    closedAt: null,
    control: null,
    editCount: 0,
    hookEvents: [],
    ...overrides,
  }
}

function renderMonitorPane(overrides: Partial<MonitorPaneProps> = {}) {
  const defaultProps: MonitorPaneProps = {
    session: createMockSession(),
    isSelected: false,
    isExpanded: false,
    isPinned: false,
    compactHeader: false,
    isVisible: true,
    onSelect: vi.fn(),
    onExpand: vi.fn(),
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
            hasUnpricedUsage: false,
            unpricedInputTokens: 0,
            unpricedOutputTokens: 0,
            unpricedCacheReadTokens: 0,
            unpricedCacheCreationTokens: 0,
            pricedTokenCoverage: 1,
            totalCostSource: 'computed_priced_tokens_full',
          },
        }),
      })

      expect(screen.getByText('$4.56')).toBeInTheDocument()
    })

    it('renders "Unavailable" when usage is unpriced and total USD is zero', () => {
      renderMonitorPane({
        session: createMockSession({
          cost: {
            totalUsd: 0,
            inputCostUsd: 0,
            outputCostUsd: 0,
            cacheReadCostUsd: 0,
            cacheCreationCostUsd: 0,
            cacheSavingsUsd: 0,
            hasUnpricedUsage: true,
            unpricedInputTokens: 10_000,
            unpricedOutputTokens: 2_000,
            unpricedCacheReadTokens: 0,
            unpricedCacheCreationTokens: 0,
            pricedTokenCoverage: 0,
            totalCostSource: 'computed_priced_tokens_partial',
          },
        }),
      })

      expect(screen.getByText('Unavailable')).toBeInTheDocument()
    })

    it('shows dash when no statusline data', () => {
      renderMonitorPane({
        session: createMockSession({ contextWindowTokens: 40000 }),
      })

      // No statuslineUsedPct → shows dash
      expect(screen.getByText('\u2014')).toBeInTheDocument()
    })

    it('renders context percentage from statusline with sky color when low', () => {
      renderMonitorPane({
        session: createMockSession({ statuslineUsedPct: 20 }),
      })

      const ctxEl = screen.getByText('20% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-sky-400')
    })

    it('renders context percentage with amber color when moderate', () => {
      renderMonitorPane({
        session: createMockSession({ statuslineUsedPct: 80 }),
      })

      const ctxEl = screen.getByText('80% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-amber-400')
    })

    it('renders context percentage with red color when high', () => {
      renderMonitorPane({
        session: createMockSession({ statuslineUsedPct: 95 }),
      })

      const ctxEl = screen.getByText('95% ctx')
      expect(ctxEl).toBeInTheDocument()
      expect(ctxEl.className).toContain('text-red-400')
    })
  })

  describe('pin indicator', () => {
    it('shows pin icon when isPinned=true', () => {
      renderMonitorPane({ isPinned: true })
      const pinButton = screen.getByTitle('Unpin pane')
      expect(pinButton.className).toContain('text-blue-500')
    })

    it('does not show pin indicator icon when isPinned=false', () => {
      renderMonitorPane({ isPinned: false })
      const pinButton = screen.getByTitle('Pin pane')
      expect(pinButton.className).toContain('text-gray-400')
    })
  })

  describe('selection ring', () => {
    it('shows blue ring when isSelected=true', () => {
      const { container } = renderMonitorPane({ isSelected: true })

      const paneDiv = container.firstElementChild
      if (!paneDiv) throw new Error('Expected pane div')
      expect(paneDiv.className).toContain('ring-2')
      expect(paneDiv.className).toContain('ring-blue-500')
    })

    it('does not show blue ring when isSelected=false', () => {
      const { container } = renderMonitorPane({ isSelected: false })

      const paneDiv = container.firstElementChild
      if (!paneDiv) throw new Error('Expected pane div')
      expect(paneDiv.className).not.toContain('ring-2')
    })
  })

  describe('click handlers', () => {
    it('calls onSelect when header is clicked', () => {
      const onSelect = vi.fn()
      renderMonitorPane({ onSelect })

      // Click on the header area — find via the cursor-pointer header div
      const header = screen.getByText('my-project').closest('[class*="cursor-pointer"]')
      if (!header) throw new Error('Expected cursor-pointer header element')
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

      const rootEl = container.firstElementChild
      if (!rootEl) throw new Error('Expected root element')
      fireEvent.contextMenu(rootEl)

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
          compactHeader={false}
          isVisible={true}
          onSelect={vi.fn()}
          onExpand={vi.fn()}
          onPin={vi.fn()}
          onHide={vi.fn()}
          onContextMenu={vi.fn()}
        >
          <div data-testid="custom-child">Custom content</div>
        </MonitorPane>,
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
