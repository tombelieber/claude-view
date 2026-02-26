import { describe, it, expect } from 'vitest'
import { renderHook } from '@testing-library/react'
import { useSubAgents } from './use-sub-agents'
import type { SubAgentInfo } from '../../types/generated/SubAgentInfo'

// Test data factory helper
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

describe('useSubAgents', () => {
  describe('empty array input', () => {
    it('returns empty arrays and zeros for all metrics', () => {
      const { result } = renderHook(() => useSubAgents([]))

      expect(result.current.all).toEqual([])
      expect(result.current.active).toEqual([])
      expect(result.current.completed).toEqual([])
      expect(result.current.errored).toEqual([])
      expect(result.current.totalCost).toBe(0)
      expect(result.current.activeCount).toBe(0)
      expect(result.current.isAnyRunning).toBe(false)
    })
  })

  describe('single agent in each status', () => {
    it('correctly categorizes a single running agent', () => {
      const agent = createAgent({ status: 'running', toolUseId: 'run_1' })
      const { result } = renderHook(() => useSubAgents([agent]))

      expect(result.current.all).toEqual([agent])
      expect(result.current.active).toEqual([agent])
      expect(result.current.completed).toEqual([])
      expect(result.current.errored).toEqual([])
      expect(result.current.activeCount).toBe(1)
      expect(result.current.isAnyRunning).toBe(true)
    })

    it('correctly categorizes a single complete agent', () => {
      const agent = createAgent({
        status: 'complete',
        toolUseId: 'comp_1',
        completedAt: 2000,
        durationMs: 1000,
      })
      const { result } = renderHook(() => useSubAgents([agent]))

      expect(result.current.all).toEqual([agent])
      expect(result.current.active).toEqual([])
      expect(result.current.completed).toEqual([agent])
      expect(result.current.errored).toEqual([])
      expect(result.current.activeCount).toBe(0)
      expect(result.current.isAnyRunning).toBe(false)
    })

    it('correctly categorizes a single error agent', () => {
      const agent = createAgent({
        status: 'error',
        toolUseId: 'err_1',
        completedAt: 1500,
        durationMs: 500,
      })
      const { result } = renderHook(() => useSubAgents([agent]))

      expect(result.current.all).toEqual([agent])
      expect(result.current.active).toEqual([])
      expect(result.current.completed).toEqual([])
      expect(result.current.errored).toEqual([agent])
      expect(result.current.activeCount).toBe(0)
      expect(result.current.isAnyRunning).toBe(false)
    })
  })

  describe('mixed status array with cost aggregation', () => {
    it('correctly filters agents into status arrays', () => {
      const agents = [
        createAgent({ status: 'running', toolUseId: 'run_1', costUsd: 0.01 }),
        createAgent({ status: 'running', toolUseId: 'run_2', costUsd: 0.02 }),
        createAgent({ status: 'complete', toolUseId: 'comp_1', costUsd: 0.05, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'complete', toolUseId: 'comp_2', costUsd: 0.10, completedAt: 2500, durationMs: 1500 }),
        createAgent({ status: 'error', toolUseId: 'err_1', costUsd: 0.03, completedAt: 1500, durationMs: 500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.all.length).toBe(5)
      expect(result.current.active.length).toBe(2)
      expect(result.current.completed.length).toBe(2)
      expect(result.current.errored.length).toBe(1)
      expect(result.current.activeCount).toBe(2)
      expect(result.current.isAnyRunning).toBe(true)

      // Verify correct agents in each array
      expect(result.current.active.map(a => a.toolUseId)).toEqual(['run_1', 'run_2'])
      expect(result.current.completed.map(a => a.toolUseId)).toEqual(['comp_1', 'comp_2'])
      expect(result.current.errored.map(a => a.toolUseId)).toEqual(['err_1'])
    })

    it('correctly sums totalCost across all agents', () => {
      const agents = [
        createAgent({ status: 'running', costUsd: 0.01 }),
        createAgent({ status: 'complete', costUsd: 0.05, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'error', costUsd: 0.03, completedAt: 1500, durationMs: 500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      // 0.01 + 0.05 + 0.03 = 0.09
      expect(result.current.totalCost).toBeCloseTo(0.09, 2)
    })

    it('handles mixed costs with varying precision', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0.12345, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'complete', costUsd: 0.67890, completedAt: 2500, durationMs: 1500 }),
        createAgent({ status: 'running', costUsd: 0.10000 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.totalCost).toBeCloseTo(0.90235, 5)
    })
  })

  describe('null cost handling', () => {
    it('treats null costUsd as 0 in sum', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0.05, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'running', costUsd: null }), // null should be treated as 0
        createAgent({ status: 'complete', costUsd: 0.03, completedAt: 2500, durationMs: 1500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      // 0.05 + 0 + 0.03 = 0.08
      expect(result.current.totalCost).toBeCloseTo(0.08, 2)
    })

    it('treats undefined costUsd as 0 in sum', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0.10, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'running', costUsd: undefined }), // undefined should be treated as 0
        createAgent({ status: 'complete', costUsd: 0.20, completedAt: 2500, durationMs: 1500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      // 0.10 + 0 + 0.20 = 0.30
      expect(result.current.totalCost).toBeCloseTo(0.30, 2)
    })

    it('handles all null/undefined costs', () => {
      const agents = [
        createAgent({ status: 'running', costUsd: null }),
        createAgent({ status: 'running', costUsd: undefined }),
        createAgent({ status: 'complete', costUsd: null, completedAt: 2000, durationMs: 1000 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.totalCost).toBe(0)
    })

    it('handles mixed null, undefined, and valid costs', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0.05, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'running', costUsd: null }),
        createAgent({ status: 'running', costUsd: undefined }),
        createAgent({ status: 'complete', costUsd: 0.03, completedAt: 2500, durationMs: 1500 }),
        createAgent({ status: 'error', costUsd: null, completedAt: 1500, durationMs: 500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      // 0.05 + 0 + 0 + 0.03 + 0 = 0.08
      expect(result.current.totalCost).toBeCloseTo(0.08, 2)
    })
  })

  describe('useMemo reference stability', () => {
    it('returns same reference for same input array', () => {
      const agents = [
        createAgent({ status: 'running', toolUseId: 'run_1' }),
        createAgent({ status: 'complete', toolUseId: 'comp_1', completedAt: 2000, durationMs: 1000 }),
      ]

      const { result, rerender } = renderHook(
        ({ subAgents }) => useSubAgents(subAgents),
        { initialProps: { subAgents: agents } }
      )

      const firstResult = result.current

      // Rerender with same array reference
      rerender({ subAgents: agents })

      // Should return same object reference (memoization working)
      expect(result.current).toBe(firstResult)
    })

    it('returns different reference for different input array', () => {
      const agents1 = [
        createAgent({ status: 'running', toolUseId: 'run_1' }),
      ]
      const agents2 = [
        createAgent({ status: 'complete', toolUseId: 'comp_1', completedAt: 2000, durationMs: 1000 }),
      ]

      const { result, rerender } = renderHook(
        ({ subAgents }) => useSubAgents(subAgents),
        { initialProps: { subAgents: agents1 } }
      )

      const firstResult = result.current

      // Rerender with different array
      rerender({ subAgents: agents2 })

      // Should return different object reference (input changed)
      expect(result.current).not.toBe(firstResult)
      expect(result.current.all).toEqual(agents2)
    })

    it('returns different reference when array contents change', () => {
      const agents1 = [
        createAgent({ status: 'running', toolUseId: 'run_1' }),
      ]
      const agents2 = [
        createAgent({ status: 'running', toolUseId: 'run_1' }),
        createAgent({ status: 'complete', toolUseId: 'comp_1', completedAt: 2000, durationMs: 1000 }),
      ]

      const { result, rerender } = renderHook(
        ({ subAgents }) => useSubAgents(subAgents),
        { initialProps: { subAgents: agents1 } }
      )

      const firstResult = result.current

      // Rerender with array with different content (even if same type)
      rerender({ subAgents: agents2 })

      // Should return different object reference
      expect(result.current).not.toBe(firstResult)
      expect(result.current.all.length).toBe(2)
      expect(firstResult.all.length).toBe(1)
    })

    it('returns same reference on parent re-render when input unchanged', () => {
      const agents = [
        createAgent({ status: 'running', toolUseId: 'run_1' }),
      ]

      let renderCount = 0
      const { result, rerender } = renderHook(
        ({ subAgents, _tick }) => {
          renderCount++
          return useSubAgents(subAgents)
        },
        { initialProps: { subAgents: agents, _tick: 0 } }
      )

      const firstResult = result.current
      expect(renderCount).toBe(1)

      // Force parent re-render by changing unrelated prop
      rerender({ subAgents: agents, _tick: 1 })

      expect(renderCount).toBe(2)
      // But result should be same reference (memoization)
      expect(result.current).toBe(firstResult)
    })
  })

  describe('edge cases', () => {
    it('handles zero cost correctly', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0, completedAt: 2000, durationMs: 1000 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.totalCost).toBe(0)
    })

    it('handles very large cost values', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 999.99, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'complete', costUsd: 1000.01, completedAt: 2500, durationMs: 1500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.totalCost).toBeCloseTo(2000.00, 2)
    })

    it('handles very small cost values', () => {
      const agents = [
        createAgent({ status: 'complete', costUsd: 0.00001, completedAt: 2000, durationMs: 1000 }),
        createAgent({ status: 'complete', costUsd: 0.00002, completedAt: 2500, durationMs: 1500 }),
      ]
      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.totalCost).toBeCloseTo(0.00003, 5)
    })

    it('handles agents with all optional fields missing', () => {
      const agent = createAgent({
        status: 'running',
        toolUseId: 'minimal',
        agentType: 'Task',
        description: 'Minimal agent',
        startedAt: 1000,
        // All optional fields undefined
        agentId: undefined,
        completedAt: undefined,
        durationMs: undefined,
        toolUseCount: undefined,
        costUsd: undefined,
      })
      const { result } = renderHook(() => useSubAgents([agent]))

      expect(result.current.all).toEqual([agent])
      expect(result.current.active).toEqual([agent])
      expect(result.current.totalCost).toBe(0)
      expect(result.current.isAnyRunning).toBe(true)
    })

    it('handles large arrays efficiently', () => {
      // Create 1000 agents with mixed statuses
      const agents = Array.from({ length: 1000 }, (_, i) => {
        const status = i % 3 === 0 ? 'running' : i % 3 === 1 ? 'complete' : 'error'
        return createAgent({
          status,
          toolUseId: `agent_${i}`,
          costUsd: 0.01,
          completedAt: status !== 'running' ? 2000 + i : undefined,
          durationMs: status !== 'running' ? 1000 : undefined,
        })
      })

      const { result } = renderHook(() => useSubAgents(agents))

      expect(result.current.all.length).toBe(1000)
      expect(result.current.active.length).toBeGreaterThan(0)
      expect(result.current.completed.length).toBeGreaterThan(0)
      expect(result.current.errored.length).toBeGreaterThan(0)
      expect(result.current.totalCost).toBeCloseTo(10.00, 2) // 1000 * 0.01
    })
  })
})
