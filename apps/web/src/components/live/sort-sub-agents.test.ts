import { describe, expect, it } from 'vitest'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'
import { sortSubAgentsForCard } from './sort-sub-agents'

// Mirrors the factory convention in use-sub-agents.test.ts
function createAgent(overrides?: Partial<SubAgentInfo>): SubAgentInfo {
  return {
    toolUseId: overrides?.toolUseId ?? 'tool_123',
    agentType: overrides?.agentType ?? 'Explore',
    description: overrides?.description ?? 'Test agent',
    status: overrides?.status ?? 'running',
    startedAt: overrides?.startedAt ?? 1000,
    agentId: overrides?.agentId,
    completedAt: overrides?.completedAt,
    durationMs: overrides?.durationMs,
    toolUseCount: overrides?.toolUseCount,
    costUsd: overrides?.costUsd,
  }
}

describe('sortSubAgentsForCard', () => {
  it('places running agents before non-running agents', () => {
    const done = createAgent({ toolUseId: 'done', status: 'complete', startedAt: 5000 })
    const running = createAgent({ toolUseId: 'run', status: 'running', startedAt: 1000 })

    const sorted = sortSubAgentsForCard([done, running])

    expect(sorted.map((a) => a.toolUseId)).toEqual(['run', 'done'])
  })

  it('orders newest-first by startedAt within the running tier', () => {
    const older = createAgent({ toolUseId: 'old', status: 'running', startedAt: 1000 })
    const newer = createAgent({ toolUseId: 'new', status: 'running', startedAt: 3000 })

    const sorted = sortSubAgentsForCard([older, newer])

    expect(sorted.map((a) => a.toolUseId)).toEqual(['new', 'old'])
  })

  it('orders newest-first by startedAt within the non-running tier', () => {
    const olderDone = createAgent({ toolUseId: 'old', status: 'complete', startedAt: 1000 })
    const newerErr = createAgent({ toolUseId: 'new', status: 'error', startedAt: 4000 })

    const sorted = sortSubAgentsForCard([olderDone, newerErr])

    expect(sorted.map((a) => a.toolUseId)).toEqual(['new', 'old'])
  })

  it('keeps the newest active agents in the first slice (overflow hides oldest)', () => {
    const agents = [
      createAgent({ toolUseId: 'a1', status: 'complete', startedAt: 1000 }),
      createAgent({ toolUseId: 'a2', status: 'complete', startedAt: 2000 }),
      createAgent({ toolUseId: 'a3', status: 'running', startedAt: 3000 }),
      createAgent({ toolUseId: 'a4', status: 'running', startedAt: 4000 }),
    ]

    const visible = sortSubAgentsForCard(agents).slice(0, 2)

    expect(visible.map((a) => a.toolUseId)).toEqual(['a4', 'a3'])
  })

  it('does not mutate the input array', () => {
    const input = [
      createAgent({ toolUseId: 'done', status: 'complete', startedAt: 5000 }),
      createAgent({ toolUseId: 'run', status: 'running', startedAt: 1000 }),
    ]
    const snapshot = input.map((a) => a.toolUseId)

    sortSubAgentsForCard(input)

    expect(input.map((a) => a.toolUseId)).toEqual(snapshot)
  })

  it('is stable for agents with the same status and startedAt (preserves spawn order)', () => {
    const first = createAgent({ toolUseId: 'first', status: 'running', startedAt: 2000 })
    const second = createAgent({ toolUseId: 'second', status: 'running', startedAt: 2000 })

    const sorted = sortSubAgentsForCard([first, second])

    expect(sorted.map((a) => a.toolUseId)).toEqual(['first', 'second'])
  })

  it('returns an empty array unchanged', () => {
    expect(sortSubAgentsForCard([])).toEqual([])
  })
})
