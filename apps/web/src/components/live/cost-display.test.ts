import { describe, expect, it } from 'vitest'
import { hasUnavailableCost, pricedCoveragePercent, unpricedTokenTotal } from './cost-display'

describe('cost-display helpers', () => {
  it('marks cost unavailable when unpriced usage exists and total tokens are non-zero', () => {
    expect(hasUnavailableCost(0, { hasUnpricedUsage: true }, 123)).toBe(true)
  })

  it('does not mark cost unavailable when total tokens are zero', () => {
    expect(hasUnavailableCost(0, { hasUnpricedUsage: true }, 0)).toBe(false)
  })

  it('does not mark cost unavailable when usage is priced', () => {
    expect(hasUnavailableCost(0, { hasUnpricedUsage: false }, 123)).toBe(false)
    expect(hasUnavailableCost(1.2, { hasUnpricedUsage: true }, 123)).toBe(false)
  })

  it('computes unpriced token totals and clamps priced coverage percent', () => {
    expect(
      unpricedTokenTotal({
        hasUnpricedUsage: true,
        unpricedInputTokens: 10,
        unpricedOutputTokens: 20,
        unpricedCacheReadTokens: 30,
        unpricedCacheCreationTokens: 40,
        pricedTokenCoverage: 0.5,
      }),
    ).toBe(100)

    expect(pricedCoveragePercent({ pricedTokenCoverage: -0.5 })).toBe(0)
    expect(pricedCoveragePercent({ pricedTokenCoverage: 0.678 })).toBe(68)
    expect(pricedCoveragePercent({ pricedTokenCoverage: 1.5 })).toBe(100)
  })
})
