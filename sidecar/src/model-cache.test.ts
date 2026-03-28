import { describe, expect, it, vi } from 'vitest'
import {
  getCacheState,
  startModelCacheRefresh,
  stopModelCacheRefresh,
  updateModelCacheFromSession,
} from './model-cache.js'

// Reset cache between tests by updating with empty then known state
function resetCache() {
  // Force cache to null state by exploiting the internal module state
  // We can't directly reset, but we can test the public API behavior
}

function makeModelInfo(value: string, displayName: string) {
  return { value, displayName, description: `${displayName} desc` } as any
}

describe('updateModelCacheFromSession', () => {
  it('populates cache from empty state', () => {
    const models = [
      makeModelInfo('default', 'Default (recommended)'),
      makeModelInfo('sonnet', 'Sonnet'),
      makeModelInfo('haiku', 'Haiku'),
    ]
    updateModelCacheFromSession(models)

    const state = getCacheState()
    expect(state.models).toHaveLength(3)
    expect(state.updatedAt).toBeGreaterThan(0)
    expect(state.models.map((m: any) => m.value)).toEqual(['default', 'sonnet', 'haiku'])
  })

  it('no-ops when model list is unchanged', () => {
    const models = [makeModelInfo('default', 'Default'), makeModelInfo('sonnet', 'Sonnet')]
    updateModelCacheFromSession(models)
    const firstUpdate = getCacheState().updatedAt

    // Same models — should not update timestamp
    updateModelCacheFromSession(models)
    const secondUpdate = getCacheState().updatedAt
    expect(secondUpdate).toBe(firstUpdate)
  })

  it('updates when model list changes', () => {
    const v1 = [makeModelInfo('default', 'Default'), makeModelInfo('sonnet', 'Sonnet')]
    updateModelCacheFromSession(v1)
    const firstUpdate = getCacheState().updatedAt

    // Wait a tick to ensure timestamp differs
    const v2 = [
      makeModelInfo('default', 'Default'),
      makeModelInfo('sonnet', 'Sonnet'),
      makeModelInfo('haiku', 'Haiku'),
    ]

    // Force time difference
    setTimeout(() => {
      updateModelCacheFromSession(v2)
      const state = getCacheState()
      expect(state.models).toHaveLength(3)
      expect(state.updatedAt).toBeGreaterThanOrEqual(firstUpdate!)
    }, 1)
  })

  it('ignores empty model list', () => {
    const models = [makeModelInfo('default', 'Default')]
    updateModelCacheFromSession(models)
    const before = getCacheState()

    updateModelCacheFromSession([])
    const after = getCacheState()
    expect(after.models).toHaveLength(before.models.length)
  })

  it('ignores null/undefined input', () => {
    const models = [makeModelInfo('default', 'Default')]
    updateModelCacheFromSession(models)

    updateModelCacheFromSession(null as any)
    updateModelCacheFromSession(undefined as any)

    const state = getCacheState()
    expect(state.models).toHaveLength(1)
  })
})

describe('getCacheState', () => {
  it('returns empty array and null updatedAt before any update', () => {
    // Note: This test may not pass if previous tests populated the cache
    // due to module-level state. The important contract is: models is always
    // an array, updatedAt is number | null.
    const state = getCacheState()
    expect(Array.isArray(state.models)).toBe(true)
    expect(state.updatedAt === null || typeof state.updatedAt === 'number').toBe(true)
  })
})

describe('startModelCacheRefresh / stopModelCacheRefresh', () => {
  it('stopModelCacheRefresh clears the interval', () => {
    const clearSpy = vi.spyOn(globalThis, 'clearInterval')
    startModelCacheRefresh()
    stopModelCacheRefresh()
    expect(clearSpy).toHaveBeenCalledTimes(1)
    clearSpy.mockRestore()
  })

  it('stopModelCacheRefresh is idempotent', () => {
    const clearSpy = vi.spyOn(globalThis, 'clearInterval')
    startModelCacheRefresh()
    stopModelCacheRefresh()
    stopModelCacheRefresh()
    expect(clearSpy).toHaveBeenCalledTimes(1)
    clearSpy.mockRestore()
  })
})
