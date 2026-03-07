import { describe, expect, it } from 'vitest'
import { getEffectiveBranch } from './effective-branch'

describe('getEffectiveBranch', () => {
  it('returns gitBranch when no worktree', () => {
    const result = getEffectiveBranch('main', null, false)
    expect(result).toEqual({ branch: 'main', driftOrigin: null, isWorktree: false })
  })

  it('returns worktreeBranch when drifted', () => {
    const result = getEffectiveBranch('main', 'feat/hook-events', true)
    expect(result).toEqual({ branch: 'feat/hook-events', driftOrigin: 'main', isWorktree: true })
  })

  it('returns worktreeBranch with no drift when gitBranch matches', () => {
    const result = getEffectiveBranch('feat/x', 'feat/x', true)
    expect(result).toEqual({ branch: 'feat/x', driftOrigin: null, isWorktree: true })
  })

  it('handles null gitBranch', () => {
    const result = getEffectiveBranch(null, 'feat/y', true)
    expect(result).toEqual({ branch: 'feat/y', driftOrigin: null, isWorktree: true })
  })

  it('handles all null', () => {
    const result = getEffectiveBranch(null, null, false)
    expect(result).toEqual({ branch: null, driftOrigin: null, isWorktree: false })
  })
})
