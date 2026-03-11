import { describe, expect, it } from 'vitest'
import { DEFAULT_LIVE_FILTERS, filterLiveSessions } from './live-filter'
import type { LiveSession } from './use-live-sessions'

/** Minimal LiveSession stub — only fields the filter actually touches */
function makeMockSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'sess-1',
    project: '-Users-test-project',
    projectDisplayName: 'test-project',
    projectPath: '/Users/test/project',
    filePath: '/tmp/session.jsonl',
    status: 'working',
    agentState: { group: 'autonomous', state: 'tool_use', label: 'Working' },
    gitBranch: 'main',
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: 'main',
    pid: 1234,
    title: 'Test session',
    lastUserMessage: 'fix the bug',
    currentActivity: 'editing file',
    turnCount: 5,
    startedAt: 1700000000,
    lastActivityAt: 1700000100,
    model: 'claude-sonnet-4-20250514',
    tokens: {
      inputTokens: 1000,
      outputTokens: 500,
      cacheReadTokens: 200,
      cacheCreationTokens: 100,
      totalTokens: 1800,
    },
    contextWindowTokens: 50000,
    cost: {
      totalUsd: 0.05,
      inputCostUsd: 0.02,
      outputCostUsd: 0.02,
      cacheReadCostUsd: 0.005,
      cacheCreationCostUsd: 0.005,
      cacheSavingsUsd: 0.01,
      hasUnpricedUsage: false,
      unpricedInputTokens: 0,
      unpricedOutputTokens: 0,
      unpricedCacheReadTokens: 0,
      unpricedCacheCreationTokens: 0,
      pricedTokenCoverage: 1.0,
      totalCostSource: 'calculated',
    },
    cacheStatus: 'warm',
    closedAt: null,
    editCount: 0,
    ...overrides,
  }
}

describe('filterLiveSessions — project filter', () => {
  const session = makeMockSession()
  const sessions = [session]

  it('project filter matches via projectPath (sidebar git_root)', () => {
    const result = filterLiveSessions(sessions, {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['/Users/test/project'],
    })
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('sess-1')
  })

  it('project filter matches via projectDisplayName', () => {
    const result = filterLiveSessions(sessions, {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['test-project'],
    })
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('sess-1')
  })

  it('project filter matches via encoded project name (fallback when displayName is empty)', () => {
    const sessionNoDisplay = makeMockSession({
      id: 'sess-encoded',
      projectDisplayName: '',
      project: '-Users-test-project',
    })
    const result = filterLiveSessions([sessionNoDisplay], {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['-Users-test-project'],
    })
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('sess-encoded')
  })

  it('encoded project name does NOT match when displayName is present (displayName takes priority)', () => {
    // When projectDisplayName is truthy, the filter uses it instead of the encoded `project`.
    // This verifies the `projectDisplayName || project` fallback semantics.
    const result = filterLiveSessions(sessions, {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['-Users-test-project'],
    })
    expect(result).toHaveLength(0)
  })

  it('project filter excludes non-matching project', () => {
    const result = filterLiveSessions(sessions, {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['/Users/other/repo'],
    })
    expect(result).toHaveLength(0)
  })

  it('project filter with multiple values matches any', () => {
    const session2 = makeMockSession({
      id: 'sess-2',
      project: '-Users-other-repo',
      projectDisplayName: 'other-repo',
      projectPath: '/Users/other/repo',
    })
    const result = filterLiveSessions([session, session2], {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['/Users/test/project', '/Users/other/repo'],
    })
    expect(result).toHaveLength(2)
  })

  it('empty projects filter returns all sessions', () => {
    const result = filterLiveSessions(sessions, {
      ...DEFAULT_LIVE_FILTERS,
      projects: [],
    })
    expect(result).toHaveLength(1)
  })

  it('projectPath match works when projectDisplayName is empty', () => {
    const sessionNoDisplay = makeMockSession({
      id: 'sess-nodisplay',
      projectDisplayName: '',
      projectPath: '/Users/test/project',
      project: '-Users-test-project',
    })
    const result = filterLiveSessions([sessionNoDisplay], {
      ...DEFAULT_LIVE_FILTERS,
      projects: ['/Users/test/project'],
    })
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('sess-nodisplay')
  })
})
