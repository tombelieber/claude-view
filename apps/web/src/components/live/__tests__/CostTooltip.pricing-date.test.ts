import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'
import { PRICING_LAST_VERIFIED } from '../CostTooltip'

/**
 * The tooltip's "Rates as of <date>" is a trust claim shown to users, but the
 * date is hardcoded in the component while the rates live in the JSON the Rust
 * binary embeds. Those two drifted a month apart once already. Pin them.
 */
describe('CostTooltip pricing date', () => {
  it('matches last_verified in data/anthropic-pricing.json', () => {
    // Vitest root is apps/web; the table lives at the repo root. A wrong path
    // throws rather than silently passing, which is the failure mode we want.
    const pricingPath = resolve(process.cwd(), '../../data/anthropic-pricing.json')
    const { last_verified } = JSON.parse(readFileSync(pricingPath, 'utf8'))
    expect(PRICING_LAST_VERIFIED).toBe(last_verified)
  })
})
