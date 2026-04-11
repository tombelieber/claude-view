import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it } from 'vitest'
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
      context: null,
    },
    gitBranch: 'main',
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: 'main',
    pid: null,
    title: 'First human prompt',
    lastUserMessage: 'Latest human prompt',
    currentActivity: '',
    turnCount: 3,
    startedAt: 1_700_000_000,
    lastActivityAt: 1_700_000_120,
    model: 'claude-sonnet-4',
    currentTurnStartedAt: null,
    lastTurnTaskSeconds: null,
    tokens: {
      inputTokens: 1_000,
      outputTokens: 500,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      cacheCreation5mTokens: 0,
      cacheCreation1hrTokens: 0,
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
    phase: { current: null, labels: [], dominant: null, freshness: 'fresh' as const },
    ...overrides,
  }
}

function renderCard(session: LiveSession) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <SessionCard session={session} currentTime={1_700_000_130} />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

describe('live SessionCard', () => {
  it('uses the last user message as the card title when available', () => {
    const { container } = renderCard(
      createMockSession({
        title: 'First human prompt about setup',
        lastUserMessage: 'Latest human prompt about auth bug',
      }),
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
      agentState: { state: 'tool_use', group: 'autonomous', label: 'Working', context: null },
    })
    renderCard(session)
    expect(screen.getByTestId('pulse-dot')).toBeInTheDocument()
  })

  it('does not show pulse dot for waiting sessions', () => {
    const session = createMockSession({
      agentState: { state: 'awaiting_input', group: 'needs_you', label: 'Waiting', context: null },
    })
    renderCard(session)
    expect(screen.queryByTestId('pulse-dot')).not.toBeInTheDocument()
  })
})

describe('SessionCard sub-agent pills', () => {
  it('shows SubAgentPills when session has sub-agents', () => {
    const session = createMockSession({
      subAgents: [
        {
          toolUseId: 'toolu_01',
          agentType: 'Explore',
          description: 'Search',
          status: 'running',
          startedAt: 1_700_000_000,
          currentActivity: 'Read',
        },
        {
          toolUseId: 'toolu_02',
          agentType: 'code-reviewer',
          description: 'Review',
          status: 'complete',
          startedAt: 1_700_000_000,
          completedAt: 1_700_000_030,
          durationMs: 30000,
        },
      ],
    })
    renderCard(session)
    expect(screen.getByText('Explore')).toBeInTheDocument()
    expect(screen.getByText('code-reviewer')).toBeInTheDocument()
  })

  it('does not show SubAgentPills when no sub-agents', () => {
    const session = createMockSession({ subAgents: undefined })
    renderCard(session)
    expect(screen.queryByText(/agents/)).not.toBeInTheDocument()
  })
})

describe('SessionCard interaction display', () => {
  it('renders SessionInteractionCard compact preview when pendingInteraction is present', () => {
    const session = createMockSession({
      pendingInteraction: {
        variant: 'permission',
        requestId: 'req-001',
        preview: 'Allow file write to src/main.rs?',
      },
    })
    renderCard(session)
    // CompactInteractionPreview shows the preview text while full data loads
    expect(screen.getByText(/Allow file write/)).toBeInTheDocument()
  })

  it('does not render interaction card when pendingInteraction is absent', () => {
    const session = createMockSession({
      pendingInteraction: undefined,
    })
    renderCard(session)
    expect(screen.queryByText(/Allow file write/)).not.toBeInTheDocument()
  })

  it('does not render interaction card when pendingInteraction is null', () => {
    const session = createMockSession({
      pendingInteraction: null,
    })
    renderCard(session)
    // No interaction-related UI should be present
    expect(screen.queryByText(/Permission/i)).not.toBeInTheDocument()
  })
})
