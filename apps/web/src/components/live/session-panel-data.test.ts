import { describe, expect, it } from 'vitest'
import type { RichSessionData } from '../../types/generated/RichSessionData'
import type { SessionDetail } from '../../types/generated/SessionDetail'
import { historyToPanelData, liveSessionToPanelData } from './session-panel-data'
import type { LiveSession } from './use-live-sessions'

function makeLiveSession(overrides: Partial<LiveSession> = {}): LiveSession {
  return {
    id: 'test-session',
    project: 'test-project',
    projectDisplayName: 'test-project',
    projectPath: '/Users/test/dev/test-project',
    filePath: '/tmp/test.jsonl',
    status: 'working',
    agentState: { group: 'autonomous', state: 'acting', label: 'Acting', context: null },
    gitBranch: 'main',
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: 'main',
    pid: 1234,
    title: 'Test',
    lastUserMessage: 'test message',
    currentActivity: 'editing',
    turnCount: 5,
    startedAt: 1700000000,
    lastActivityAt: 1700001000,
    model: 'claude-sonnet-4-20250514',
    currentTurnStartedAt: null,
    lastTurnTaskSeconds: null,
    tokens: {
      inputTokens: 1000,
      outputTokens: 500,
      cacheReadTokens: 200,
      cacheCreationTokens: 100,
      cacheCreation5mTokens: 50,
      cacheCreation1hrTokens: 50,
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

describe('liveSessionToPanelData', () => {
  it('wires all statusline fields', () => {
    const session = makeLiveSession({
      statuslineContextWindowSize: 1_000_000,
      statuslineUsedPct: 42.5,
      statuslineCostUsd: 1.23,
      statuslineTotalDurationMs: BigInt(60000),
      statuslineLinesAdded: BigInt(100),
      statuslineLinesRemoved: BigInt(20),
      statuslineInputTokens: BigInt(5000),
      statuslineOutputTokens: BigInt(2000),
      statuslineCacheReadTokens: BigInt(1000),
      statuslineCacheCreationTokens: BigInt(500),
      statuslineCwd: '/Users/test/dev',
      statuslineProjectDir: '/Users/test/dev/project',
      modelDisplayName: 'Sonnet',
    })

    const data = liveSessionToPanelData(session)

    expect(data.statuslineContextWindowSize).toBe(1_000_000)
    expect(data.statuslineUsedPct).toBe(42.5)
    expect(data.statuslineCostUsd).toBe(1.23)
    expect(data.statuslineTotalDurationMs).toBe(60000)
    expect(data.statuslineLinesAdded).toBe(100)
    expect(data.statuslineLinesRemoved).toBe(20)
    expect(data.statuslineInputTokens).toBe(5000)
    expect(data.statuslineOutputTokens).toBe(2000)
    expect(data.statuslineCacheReadTokens).toBe(1000)
    expect(data.statuslineCacheCreationTokens).toBe(500)
    expect(data.statuslineCwd).toBe('/Users/test/dev')
    expect(data.statuslineProjectDir).toBe('/Users/test/dev/project')
    expect(data.modelDisplayName).toBe('Sonnet')
  })

  it('maps null statusline fields to null', () => {
    const session = makeLiveSession()
    const data = liveSessionToPanelData(session)

    expect(data.statuslineContextWindowSize).toBeNull()
    expect(data.statuslineUsedPct).toBeNull()
    expect(data.statuslineCostUsd).toBeNull()
    expect(data.statuslineTotalDurationMs).toBeNull()
    expect(data.statuslineLinesAdded).toBeNull()
    expect(data.statuslineLinesRemoved).toBeNull()
    expect(data.statuslineInputTokens).toBeNull()
    expect(data.statuslineOutputTokens).toBeNull()
    expect(data.statuslineCacheReadTokens).toBeNull()
    expect(data.statuslineCacheCreationTokens).toBeNull()
    expect(data.statuslineCwd).toBeNull()
    expect(data.statuslineProjectDir).toBeNull()
    expect(data.modelDisplayName).toBeNull()
  })

  it('model display name priority — live session', () => {
    const session = makeLiveSession({
      model: 'claude-sonnet-4-20250514',
      modelDisplayName: 'Sonnet',
    })
    const data = liveSessionToPanelData(session)
    expect(data.modelDisplayName).toBe('Sonnet')
    expect(data.model).toBe('claude-sonnet-4-20250514')
  })
})

describe('historyToPanelData', () => {
  it('sets all statusline fields to null', () => {
    const sessionDetail = {
      id: 'hist-1',
      project: 'proj',
      displayName: 'proj',
      projectPath: '/path',
      gitBranch: 'main',
      turnCount: 10,
      modifiedAt: 1700000000,
      totalInputTokens: 1000,
      totalOutputTokens: 500,
      totalCacheReadTokens: 100,
      totalCacheCreationTokens: 50,
    } as unknown as SessionDetail

    const data = historyToPanelData(sessionDetail, undefined, undefined)

    expect(data.modelDisplayName).toBeNull()
    expect(data.statuslineCostUsd).toBeNull()
    expect(data.statuslineTotalDurationMs).toBeNull()
    expect(data.statuslineLinesAdded).toBeNull()
    expect(data.statuslineLinesRemoved).toBeNull()
    expect(data.statuslineInputTokens).toBeNull()
    expect(data.statuslineOutputTokens).toBeNull()
    expect(data.statuslineCacheReadTokens).toBeNull()
    expect(data.statuslineCacheCreationTokens).toBeNull()
    expect(data.statuslineCwd).toBeNull()
    expect(data.statuslineProjectDir).toBeNull()
  })

  it('model display name is null for history sessions', () => {
    const sessionDetail = {
      id: 'hist-2',
      project: 'proj',
      displayName: 'proj',
      projectPath: '/path',
      gitBranch: null,
      turnCount: 5,
      modifiedAt: 1700000000,
    } as unknown as SessionDetail

    const richData = {
      model: 'claude-opus-4-6',
    } as unknown as RichSessionData

    const data = historyToPanelData(sessionDetail, richData, undefined)
    expect(data.model).toBe('claude-opus-4-6')
    expect(data.modelDisplayName).toBeNull()
  })
})
