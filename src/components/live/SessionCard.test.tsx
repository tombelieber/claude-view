import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

function createMockSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'session-1',
    project: 'test-project',
    projectDisplayName: 'test-project',
    projectPath: '/Users/test/dev/test-project',
    filePath: '/tmp/session-1.jsonl',
    status: 'paused',
    agentState: {
      group: 'needs_you',
      state: 'awaiting_input',
      label: 'Waiting for your next message',
    },
    gitBranch: 'main',
    pid: null,
    title: 'First human prompt',
    lastUserMessage: 'Latest human prompt',
    currentActivity: '',
    turnCount: 3,
    startedAt: 1_700_000_000,
    lastActivityAt: 1_700_000_120,
    model: 'claude-sonnet-4',
    tokens: {
      inputTokens: 1_000,
      outputTokens: 500,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalTokens: 1_500,
    },
    contextWindowTokens: 1_500,
    cost: {
      totalUsd: 0.42,
      inputCostUsd: 0.2,
      outputCostUsd: 0.2,
      cacheReadCostUsd: 0,
      cacheCreationCostUsd: 0,
      cacheSavingsUsd: 0,
      isEstimated: false,
    },
    cacheStatus: 'warm',
    ...overrides,
  }
}

function renderCard(session: LiveSession) {
  return render(
    <MemoryRouter>
      <SessionCard session={session} currentTime={1_700_000_130} />
    </MemoryRouter>
  )
}

describe('live SessionCard', () => {
  it('uses the last user message as the card title when available', () => {
    const { container } = renderCard(
      createMockSession({
        title: 'First human prompt about setup',
        lastUserMessage: 'Latest human prompt about auth bug',
      })
    )

    const title = container.querySelector('p.text-sm.font-medium')
    expect(title).toBeInTheDocument()
    expect(title).toHaveTextContent('Latest human prompt about auth bug')
    expect(title).not.toHaveTextContent('First human prompt about setup')
  })
})

describe('SessionCard pulse dot', () => {
  it('shows pulse dot for autonomous (running) sessions', () => {
    const session = createMockSession({
      status: 'working',
      agentState: { state: 'tool_use', group: 'autonomous', label: 'Working' },
    })
    renderCard(session)
    expect(screen.getByTestId('pulse-dot')).toBeInTheDocument()
  })

  it('does not show pulse dot for waiting sessions', () => {
    const session = createMockSession({
      agentState: { state: 'awaiting_input', group: 'needs_you', label: 'Waiting' },
    })
    renderCard(session)
    expect(screen.queryByTestId('pulse-dot')).not.toBeInTheDocument()
  })
})

describe('SessionCard sub-agent pills', () => {
  it('shows SubAgentPills when session has sub-agents', () => {
    const session = createMockSession({
      subAgents: [
        { toolUseId: 'toolu_01', agentType: 'Explore', description: 'Search', status: 'running', startedAt: 1_700_000_000, currentActivity: 'Read' },
        { toolUseId: 'toolu_02', agentType: 'code-reviewer', description: 'Review', status: 'complete', startedAt: 1_700_000_000, completedAt: 1_700_000_030, durationMs: 30000 },
      ],
    })
    renderCard(session)
    expect(screen.getByText('E')).toBeInTheDocument()
    expect(screen.getByText('2 agents (1 active)')).toBeInTheDocument()
  })

  it('does not show SubAgentPills when no sub-agents', () => {
    const session = createMockSession({ subAgents: undefined })
    renderCard(session)
    expect(screen.queryByText(/agents/)).not.toBeInTheDocument()
  })
})

describe('SessionCard question card', () => {
  it('shows QuestionCard when state is awaiting_input with question context', () => {
    const session = createMockSession({
      agentState: {
        group: 'needs_you',
        state: 'awaiting_input',
        label: 'Asked you a question',
        context: {
          questions: [{
            question: 'Which database should we use?',
            header: 'DB',
            options: [
              { label: 'PostgreSQL', description: 'Relational' },
              { label: 'SQLite', description: 'Embedded' },
            ],
            multiSelect: false,
          }],
        },
      },
    })
    renderCard(session)
    expect(screen.getByTestId('question-card')).toBeInTheDocument()
    expect(screen.getByText('Which database should we use?')).toBeInTheDocument()
    expect(screen.getByText('PostgreSQL')).toBeInTheDocument()
  })

  it('does not show QuestionCard when awaiting_input but no context', () => {
    const session = createMockSession({
      agentState: {
        group: 'needs_you',
        state: 'awaiting_input',
        label: 'Waiting for your next message',
      },
    })
    renderCard(session)
    expect(screen.queryByTestId('question-card')).not.toBeInTheDocument()
  })

  it('does not show QuestionCard when autonomous state', () => {
    const session = createMockSession({
      agentState: {
        group: 'autonomous',
        state: 'acting',
        label: 'Using tools...',
      },
    })
    renderCard(session)
    expect(screen.queryByTestId('question-card')).not.toBeInTheDocument()
  })
})
