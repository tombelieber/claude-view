import { describe, expect, it } from 'vitest'
import { getContextLimit } from './model-context-windows'

describe('getContextLimit', () => {
  describe('200K default — fill within normal range', () => {
    it('returns 200K when no args provided', () => {
      expect(getContextLimit()).toBe(200_000)
    })

    it('returns 200K for a known model with no fill', () => {
      expect(getContextLimit('claude-sonnet-4-6')).toBe(200_000)
    })

    it('returns 200K when fill is exactly 200K (boundary)', () => {
      expect(getContextLimit('claude-sonnet-4-6', 200_000)).toBe(200_000)
    })

    it('returns 200K when fill is just under 200K', () => {
      expect(getContextLimit('claude-sonnet-4-6', 199_999)).toBe(200_000)
    })

    it('returns 200K for unknown model with low fill', () => {
      expect(getContextLimit('unknown-model-xyz', 50_000)).toBe(200_000)
    })
  })

  describe('1M inference — fill exceeds 200K', () => {
    // REGRESSION: this is the exact bug that was filed.
    // A session using the context-1m-2025-08-07 beta header can accumulate
    // >200K input tokens in a single turn. Without this fix, the gauge showed
    // >100% usage, making the gauge meaningless.
    it('returns 1M when fill exceeds 200K (regression: 1M session bug)', () => {
      expect(getContextLimit('claude-sonnet-4-6', 200_001)).toBe(1_000_000)
    })

    it('returns 1M when fill is 250K (typical mid-session 1M usage)', () => {
      expect(getContextLimit('claude-sonnet-4-6', 250_000)).toBe(1_000_000)
    })

    it('returns 1M when fill is exactly 1M (full 1M context)', () => {
      expect(getContextLimit('claude-opus-4-6', 1_000_000)).toBe(1_000_000)
    })

    it('returns 1M regardless of model name when fill > 200K', () => {
      // Fill is the proof — model name is irrelevant for 1M detection
      expect(getContextLimit('claude-haiku-4-5', 300_000)).toBe(1_000_000)
      expect(getContextLimit(null, 300_000)).toBe(1_000_000)
      expect(getContextLimit(undefined, 300_000)).toBe(1_000_000)
    })
  })

  describe('statuslineSize — authoritative value takes precedence', () => {
    it('returns 1M when statuslineSize is 1_000_000', () => {
      expect(getContextLimit(undefined, undefined, 1_000_000)).toBe(1_000_000)
    })

    it('statuslineSize takes precedence over currentFill > 200K', () => {
      // statuslineSize says 200K, but fill is 250K (would infer 1M without statusline)
      // statuslineSize wins — it's the authoritative source
      expect(getContextLimit('claude-sonnet-4-6', 250_000, 200_000)).toBe(200_000)
    })

    it('statuslineSize=0 falls through to fill-based logic', () => {
      // statuslineSize=0 is treated as absent, so fill > 200K triggers 1M inference
      expect(getContextLimit('claude-sonnet-4-6', 250_000, 0)).toBe(1_000_000)
    })

    it('statuslineSize=null falls through to fill-based logic', () => {
      expect(getContextLimit('claude-sonnet-4-6', 250_000, null)).toBe(1_000_000)
    })

    it('statuslineSize=undefined falls through to fill-based logic', () => {
      expect(getContextLimit('claude-sonnet-4-6', 250_000, undefined)).toBe(1_000_000)
    })

    it('returns statuslineSize even with low fill', () => {
      // 1M statusline with only 50K fill — statusline still wins
      expect(getContextLimit('claude-sonnet-4-6', 50_000, 1_000_000)).toBe(1_000_000)
    })
  })

  describe('usedPct never exceeds 100% — the visible regression', () => {
    // This test encodes the user-visible symptom: the gauge bar and "X% used"
    // text should never show >100%. Before the fix, a 250K-fill session on a
    // 200K denominator produced 125%, which broke the gauge bar and the label.
    it('usedPct stays ≤ 100% for a 250K-fill 1M session', () => {
      const fill = 250_000
      const limit = getContextLimit('claude-sonnet-4-6', fill)
      const usedPct = Math.min((fill / limit) * 100, 100)
      expect(usedPct).toBeLessThanOrEqual(100)
      expect(usedPct).toBeCloseTo(25, 0) // 250K / 1M = 25%
    })

    it('usedPct is correct for a normal 80K-fill 200K session', () => {
      const fill = 80_000
      const limit = getContextLimit('claude-sonnet-4-6', fill)
      const usedPct = (fill / limit) * 100
      expect(usedPct).toBe(40)
    })
  })
})
