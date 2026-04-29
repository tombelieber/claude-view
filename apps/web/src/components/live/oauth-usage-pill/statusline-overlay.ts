import type { UsageTier } from '../../../hooks/use-oauth-usage'
import type { StatuslineRateLimit } from './types'

/** Convert Unix seconds to ISO-8601 string for tier resetAt. */
export function unixSecsToIso(secs: number): string {
  return new Date(secs * 1000).toISOString()
}

/**
 * Backward-compat shim: backends pre-2026-04-30 don't emit `kind`. Infer it
 * from the id so the rest of the UI can rely on `kind` being present.
 */
function inferKind(tier: UsageTier): NonNullable<UsageTier['kind']> {
  if (tier.kind) return tier.kind
  if (tier.id === 'session' || tier.id === 'five_hour') return 'session'
  if (tier.id === 'extra') return 'extra'
  if (tier.id === 'weekly' || tier.id === 'weekly_sonnet' || tier.id.startsWith('seven_day'))
    return 'window'
  return 'other'
}

/** Hydrate `kind` so consumers can group without ambiguity, regardless of backend version. */
export function withInferredKind(tiers: UsageTier[]): UsageTier[] {
  return tiers.map((t) => (t.kind ? t : { ...t, kind: inferKind(t) }))
}

/** Find the session tier (new id `five_hour`, legacy id `session`), or fall back to the first tier. */
export function getSessionTier(tiers: UsageTier[]): UsageTier | undefined {
  return tiers.find((t) => t.id === 'session' || t.id === 'five_hour') ?? tiers[0]
}

/**
 * Overlay real-time statusline rate-limit data onto API-polled tiers.
 *
 * The statusline event stream is the *primary* source for percentage and reset
 * time on the 5h + 7d windows because it updates within seconds of usage.
 * The polled API still wins for tiers the statusline doesn't cover (`extra`,
 * the new model-specific 7d windows like `seven_day_opus`, …) and supplies
 * the human label + `spent` string the statusline never carries.
 */
export function applyStatuslineOverrides(
  apiTiers: UsageTier[],
  statusline: StatuslineRateLimit | null | undefined,
): UsageTier[] {
  if (!statusline) return apiTiers

  const tiers = apiTiers.map((tier) => {
    if (tier.id === 'session' || tier.id === 'five_hour') {
      return {
        ...tier,
        percentage: statusline.pct5h,
        resetAt: unixSecsToIso(statusline.reset5h),
      }
    }
    if (
      (tier.id === 'weekly' || tier.id === 'seven_day' || tier.id === 'weekly_sonnet') &&
      statusline.pct7d != null
    ) {
      return {
        ...tier,
        percentage: statusline.pct7d,
        resetAt: statusline.reset7d != null ? unixSecsToIso(statusline.reset7d) : tier.resetAt,
      }
    }
    return tier
  })

  // If the API returned nothing but the statusline has live data, synthesize
  // a session tier so the pill still renders with a useful number.
  if (tiers.length === 0) {
    tiers.push({
      id: 'five_hour',
      label: 'Session (5hr)',
      kind: 'session',
      percentage: statusline.pct5h,
      resetAt: unixSecsToIso(statusline.reset5h),
    })
  }

  return tiers
}
