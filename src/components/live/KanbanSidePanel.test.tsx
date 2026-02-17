import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { KanbanSidePanel } from './KanbanSidePanel'
import type { LiveSession } from './use-live-sessions'

// Mock terminal pane (creates WebSocket)
vi.mock('./RichTerminalPane', () => ({
  RichTerminalPane: ({ sessionId }: { sessionId: string }) => (
    <div data-testid="terminal-pane">Terminal: {sessionId}</div>
  ),
}))

// Mock SwimLanes
vi.mock('./SwimLanes', () => ({
  SwimLanes: ({ subAgents }: { subAgents: unknown[] }) => (
    <div data-testid="swim-lanes">{subAgents.length} sub-agents</div>
  ),
}))

// Mock SubAgentDrillDown
vi.mock('./SubAgentDrillDown', () => ({
  SubAgentDrillDown: ({ agentId }: { agentId: string }) => (
    <div data-testid="drill-down">DrillDown: {agentId}</div>
  ),
}))

function makeSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'test-1',
    project: 'test-project',
    projectDisplayName: 'test-project',
    projectPath: '/path',
    filePath: '/path/to/file.jsonl',
    status: 'working',
    agentState: { state: 'tool_use', group: 'autonomous', label: 'Working', confidence: 1.0, source: 'jsonl' },
    gitBranch: 'feature/auth',
    pid: null,
    title: 'Test session',
    lastUserMessage: 'Add auth module',
    currentActivity: '',
    turnCount: 12,
    startedAt: 1_700_000_000,
    lastActivityAt: 1_700_000_120,
    model: 'claude-sonnet-4-5-20250929',
    tokens: { inputTokens: 10000, outputTokens: 5000, cacheReadTokens: 0, cacheCreationTokens: 0, totalTokens: 15000 },
    contextWindowTokens: 100000,
    cost: { totalUsd: 2.34, inputCostUsd: 1.50, outputCostUsd: 0.84, cacheReadCostUsd: 0, cacheCreationCostUsd: 0, cacheSavingsUsd: 0 },
    cacheStatus: 'warm',
    ...overrides,
  }
}

describe('KanbanSidePanel', () => {
  it('renders session header with project and branch', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByText('test-project')).toBeInTheDocument()
    expect(screen.getByText('feature/auth')).toBeInTheDocument()
  })

  it('renders 4 tab buttons', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByRole('tab', { name: /terminal/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /sub-agents/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /timeline/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /cost/i })).toBeInTheDocument()
  })

  it('shows Terminal tab by default', () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    expect(screen.getByTestId('terminal-pane')).toBeInTheDocument()
  })

  it('switches to Cost tab on click', async () => {
    render(<KanbanSidePanel session={makeSession()} onClose={vi.fn()} />)
    await userEvent.click(screen.getByRole('tab', { name: /cost/i }))
    expect(screen.getByText('Total Cost')).toBeInTheDocument()
  })

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn()
    render(<KanbanSidePanel session={makeSession()} onClose={onClose} />)
    await userEvent.click(screen.getByRole('button', { name: /close/i }))
    expect(onClose).toHaveBeenCalled()
  })

  it('calls onClose when Escape pressed', async () => {
    const onClose = vi.fn()
    render(<KanbanSidePanel session={makeSession()} onClose={onClose} />)
    await userEvent.keyboard('{Escape}')
    expect(onClose).toHaveBeenCalled()
  })

  it('defaults to Sub-Agents tab when session has sub-agents', () => {
    const session = makeSession({
      subAgents: [
        { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'running', startedAt: 1_700_000_000 },
      ],
    })
    render(<KanbanSidePanel session={session} onClose={vi.fn()} />)
    const subAgentsTab = screen.getByRole('tab', { name: /sub-agents/i })
    expect(subAgentsTab).toHaveAttribute('aria-selected', 'true')
  })
})
