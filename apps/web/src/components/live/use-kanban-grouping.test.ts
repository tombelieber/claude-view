import { describe, expect, it } from 'vitest'
import { groupSessionsByProjectBranch } from './use-kanban-grouping'
import type { LiveSession } from './use-live-sessions'

function makeSession(overrides: Partial<LiveSession> & { id: string }): LiveSession {
  return {
    project: 'test-project',
    projectDisplayName: 'test-project',
    projectPath: '/Users/test/test-project',
    filePath: '',
    status: 'working',
    agentState: { group: 'autonomous', state: 'thinking', label: 'Thinking' },
    gitBranch: 'main',
    pid: 1234,
    title: 'Test',
    lastUserMessage: 'test',
    currentActivity: 'thinking',
    turnCount: 1,
    startedAt: 1000,
    lastActivityAt: 2000,
    model: 'claude-sonnet-4-6',
    tokens: {
      inputTokens: 100,
      outputTokens: 50,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
      totalTokens: 150,
    },
    contextWindowTokens: 200000,
    cost: {
      totalUsd: 0.01,
      inputCostUsd: 0.005,
      outputCostUsd: 0.005,
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
    cacheStatus: 'unknown',
    ...overrides,
  }
}

describe('groupSessionsByProjectBranch', () => {
  it('returns empty array for no sessions', () => {
    const result = groupSessionsByProjectBranch([])
    expect(result).toEqual([])
  })

  it('groups sessions by project then branch', () => {
    const sessions = [
      makeSession({
        id: 's1',
        projectDisplayName: 'claude-view',
        gitBranch: 'main',
        lastActivityAt: 3000,
      }),
      makeSession({
        id: 's2',
        projectDisplayName: 'claude-view',
        gitBranch: 'feat-groups',
        lastActivityAt: 2000,
      }),
      makeSession({
        id: 's3',
        projectDisplayName: 'my-api',
        gitBranch: 'main',
        lastActivityAt: 1000,
      }),
    ]

    const result = groupSessionsByProjectBranch(sessions)

    expect(result).toHaveLength(2)
    expect(result[0].projectName).toBe('claude-view')
    expect(result[0].branches).toHaveLength(2)
    expect(result[0].branches[0].branchName).toBe('main')
    expect(result[0].branches[0].sessions).toHaveLength(1)
    expect(result[0].branches[1].branchName).toBe('feat-groups')
    expect(result[0].branches[1].sessions).toHaveLength(1)
    expect(result[1].projectName).toBe('my-api')
    expect(result[1].branches).toHaveLength(1)
  })

  it('sorts projects by most recent activity descending', () => {
    const sessions = [
      makeSession({ id: 's1', projectDisplayName: 'aaa-project', lastActivityAt: 1000 }),
      makeSession({ id: 's2', projectDisplayName: 'zzz-project', lastActivityAt: 5000 }),
    ]

    const result = groupSessionsByProjectBranch(sessions)
    expect(result[0].projectName).toBe('zzz-project')
    expect(result[1].projectName).toBe('aaa-project')
  })

  it('sorts branches within project by most recent activity descending', () => {
    const sessions = [
      makeSession({
        id: 's1',
        projectDisplayName: 'proj',
        gitBranch: 'alpha',
        lastActivityAt: 1000,
      }),
      makeSession({
        id: 's2',
        projectDisplayName: 'proj',
        gitBranch: 'beta',
        lastActivityAt: 5000,
      }),
    ]

    const result = groupSessionsByProjectBranch(sessions)
    expect(result[0].branches[0].branchName).toBe('beta')
    expect(result[0].branches[1].branchName).toBe('alpha')
  })

  it('computes aggregate cost per project', () => {
    const sessions = [
      makeSession({
        id: 's1',
        projectDisplayName: 'proj',
        gitBranch: 'main',
        cost: { ...makeSession({ id: 'x' }).cost, totalUsd: 1.5 },
      }),
      makeSession({
        id: 's2',
        projectDisplayName: 'proj',
        gitBranch: 'dev',
        cost: { ...makeSession({ id: 'x' }).cost, totalUsd: 2.5 },
      }),
    ]

    const result = groupSessionsByProjectBranch(sessions)
    expect(result[0].totalCostUsd).toBeCloseTo(4.0)
  })

  it('handles sessions with null gitBranch', () => {
    const sessions = [makeSession({ id: 's1', projectDisplayName: 'proj', gitBranch: null })]

    const result = groupSessionsByProjectBranch(sessions)
    expect(result[0].branches[0].branchName).toBeNull()
    expect(result[0].branches[0].sessions).toHaveLength(1)
  })
})
