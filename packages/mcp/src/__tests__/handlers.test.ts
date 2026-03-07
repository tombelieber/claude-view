import { describe, expect, it, mock } from 'bun:test'
import type { ClaudeViewClient } from '../client.js'
import { liveTools } from '../tools/live.js'
import { sessionTools } from '../tools/sessions.js'
import { statsTools } from '../tools/stats.js'

function mockClient(response: unknown): ClaudeViewClient {
  return {
    baseUrl: 'http://localhost:47892',
    get: mock(() => Promise.resolve(response)),
  } as unknown as ClaudeViewClient
}

function assertNoUndefinedValues(obj: Record<string, unknown>, path = '') {
  for (const [key, value] of Object.entries(obj)) {
    const fullPath = path ? `${path}.${key}` : key
    if (value === undefined) {
      throw new Error(
        `Field "${fullPath}" is undefined — likely a field name mismatch with the API`,
      )
    }
  }
}

describe('handler integration — sessions', () => {
  const API_SESSION = {
    id: 'sess-001',
    displayName: 'my-project',
    project: '/Users/test/my-project',
    gitBranch: 'feat/auth',
    primaryModel: 'claude-sonnet-4-6',
    turnCount: 12,
    messageCount: 24,
    commitCount: 3,
    durationSeconds: 1800,
    totalInputTokens: 50000,
    totalOutputTokens: 15000,
    totalCacheReadTokens: 8000,
    modifiedAt: 1709136000,
    preview: 'Implemented OAuth2 login flow',
    userPromptCount: 10,
    derivedMetrics: {
      tokensPerPrompt: 5000,
      reeditRate: 0.08,
      toolDensity: 0.6,
      editVelocity: 3.2,
      readToEditRatio: 2.1,
    },
    commits: [
      { hash: 'abc12345def67890', message: 'feat: add OAuth2', branch: 'feat/auth' },
      { hash: 'def67890abc12345', message: 'fix: token refresh', branch: 'feat/auth' },
    ],
  }

  it('list_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({ sessions: [API_SESSION], total: 1, hasMore: false })
    const tool = sessionTools.find((t) => t.name === 'list_sessions')!
    const result = JSON.parse(await tool.handler(client, {}))
    expect(result.sessions).toHaveLength(1)
    const s = result.sessions[0]
    assertNoUndefinedValues(s)
    expect(s.id).toBe('sess-001')
    expect(s.project).toBe('my-project')
    expect(s.branch).toBe('feat/auth')
    expect(s.model).toBe('claude-sonnet-4-6')
    expect(s.turns).toBe(12)
    expect(s.input_tokens).toBe(50000)
    expect(s.output_tokens).toBe(15000)
    expect(s.duration_min).toBe(30)
    expect(result.total).toBe(1)
    expect(result.has_more).toBe(false)
  })

  it('get_session maps camelCase API fields correctly', async () => {
    const client = mockClient(API_SESSION)
    const tool = sessionTools.find((t) => t.name === 'get_session')!
    const result = JSON.parse(await tool.handler(client, { session_id: 'sess-001' }))
    assertNoUndefinedValues(result)
    expect(result.project).toBe('my-project')
    expect(result.summary).toBe('Implemented OAuth2 login flow')
    expect(result.derived_metrics.tokensPerPrompt).toBe(5000)
    expect(result.recent_commits[0].hash).toBe('abc12345')
    expect(result.cache_read_tokens).toBe(8000)
  })

  it('search_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({
      query: 'OAuth',
      totalSessions: 1,
      totalMatches: 3,
      elapsedMs: 12.5,
      sessions: [
        {
          sessionId: 'sess-001',
          project: 'my-project',
          branch: 'feat/auth',
          matchCount: 3,
          bestScore: 0.95,
          matches: [
            { role: 'assistant', snippet: 'Implementing <mark>OAuth</mark> flow', turnNumber: 5 },
          ],
        },
      ],
    })
    const tool = sessionTools.find((t) => t.name === 'search_sessions')!
    const result = JSON.parse(await tool.handler(client, { query: 'OAuth' }))
    expect(result.total_sessions).toBe(1)
    expect(result.total_matches).toBe(3)
    expect(result.elapsed_ms).toBe(12.5)
    const r = result.results[0]
    assertNoUndefinedValues(r)
    expect(r.session_id).toBe('sess-001')
    expect(r.match_count).toBe(3)
    expect(r.top_matches[0].snippet).toContain('**OAuth**')
  })
})

describe('handler integration — stats', () => {
  it('get_stats maps camelCase API fields correctly', async () => {
    const client = mockClient({
      totalSessions: 150,
      totalProjects: 8,
      topProjects: [{ name: 'proj', displayName: 'My Project', sessionCount: 42 }],
      topSkills: [{ name: 'commit', count: 100 }],
      toolTotals: { edit: 500, read: 1200, bash: 300, write: 80 },
      currentWeek: { sessionCount: 12, totalTokens: 500000, totalFilesEdited: 45, commitCount: 8 },
      trends: { sessions: { current: 12, previous: 10, delta: 2, deltaPercent: 20.0 } },
    })
    const tool = statsTools.find((t) => t.name === 'get_stats')!
    const result = JSON.parse(await tool.handler(client, {}))
    assertNoUndefinedValues(result)
    expect(result.total_sessions).toBe(150)
    expect(result.total_projects).toBe(8)
    expect(result.top_projects[0].name).toBe('My Project')
    expect(result.top_projects[0].sessions).toBe(42)
    expect(result.current_week.sessionCount).toBe(12)
    expect(result.tool_totals.edit).toBe(500)
  })

  it('get_fluency_score maps flat API fields correctly', async () => {
    const client = mockClient({
      score: 78,
      achievementRate: 0.85,
      frictionRate: 0.12,
      costEfficiency: 0.5,
      satisfactionTrend: 0.7,
      consistency: 0.5,
      sessionsAnalyzed: 42,
    })
    const tool = statsTools.find((t) => t.name === 'get_fluency_score')!
    const result = JSON.parse(await tool.handler(client, {}))
    assertNoUndefinedValues(result)
    expect(result.score).toBe(78)
    expect(result.achievementRate).toBe(0.85)
    expect(result.frictionRate).toBe(0.12)
    expect(result.sessionsAnalyzed).toBe(42)
  })

  it('get_token_stats passes through API response', async () => {
    const apiResponse = {
      totalInputTokens: 2000000,
      totalOutputTokens: 600000,
      totalCacheReadTokens: 500000,
      totalCacheCreationTokens: 100000,
      cacheHitRatio: 0.45,
      turnsCount: 800,
      sessionsCount: 150,
    }
    const client = mockClient(apiResponse)
    const tool = statsTools.find((t) => t.name === 'get_token_stats')!
    const result = JSON.parse(await tool.handler(client, {}))
    expect(result.totalInputTokens).toBe(2000000)
    expect(result.cacheHitRatio).toBe(0.45)
    expect(result.sessionsCount).toBe(150)
  })
})

describe('handler integration — live', () => {
  it('list_live_sessions maps camelCase API fields correctly', async () => {
    const client = mockClient({
      sessions: [
        {
          id: 'live-001',
          projectDisplayName: 'my-project',
          agentState: { group: 'working', label: 'Editing files', icon: '✏️' },
          model: 'claude-sonnet-4-6',
          turnCount: 5,
          cost: { totalUsd: 0.42 },
          tokens: { totalTokens: 150000 },
          startedAt: 1709136000,
          lastActivityAt: 1709137800,
          subAgents: [{ id: 'sub-1' }],
        },
      ],
      total: 1,
      processCount: 1,
    })
    const tool = liveTools.find((t) => t.name === 'list_live_sessions')!
    const result = JSON.parse(await tool.handler(client, {}))
    expect(result.sessions).toHaveLength(1)
    const s = result.sessions[0]
    assertNoUndefinedValues(s)
    expect(s.project).toBe('my-project')
    expect(s.agent_state).toBe('Editing files')
    expect(s.cost_usd).toBe(0.42)
    expect(s.total_tokens).toBe(150000)
    expect(s.sub_agents).toBe(1)
    expect(result.process_count).toBe(1)
  })

  it('get_live_summary maps camelCase API fields correctly', async () => {
    const client = mockClient({
      needsYouCount: 2,
      autonomousCount: 3,
      deliveredCount: 1,
      totalCostTodayUsd: 4.56,
      totalTokensToday: 1500000,
      processCount: 6,
    })
    const tool = liveTools.find((t) => t.name === 'get_live_summary')!
    const result = JSON.parse(await tool.handler(client, {}))
    assertNoUndefinedValues(result)
    expect(result.needs_attention).toBe(2)
    expect(result.autonomous).toBe(3)
    expect(result.delivered).toBe(1)
    expect(result.total_cost_today_usd).toBe(4.56)
    expect(result.total_tokens_today).toBe(1500000)
    expect(result.process_count).toBe(6)
  })
})
