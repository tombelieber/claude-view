import * as Tooltip from '@radix-ui/react-tooltip'
import { RefreshCw } from 'lucide-react'
import { useState } from 'react'
import { useAuthIdentity } from '../../hooks/use-auth-identity'
import { type UsageTier, useOAuthUsage } from '../../hooks/use-oauth-usage'

/** Human-readable reset countdown from an ISO date. */
function formatReset(resetAt: string): string {
  if (!resetAt) return '--'
  const diffMs = new Date(resetAt).getTime() - Date.now()
  if (diffMs <= 0) return 'now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  if (hours < 24) return `${hours}h`
  return `${Math.ceil(diffMs / (1000 * 60 * 60 * 24))}d`
}

/** Human-readable "Updated Xs ago" from a millisecond epoch timestamp. */
function formatUpdatedAgo(epochMs: number): string {
  if (!epochMs) return ''
  const diffMs = Date.now() - epochMs
  if (diffMs < 5_000) return 'Updated just now'
  const secs = Math.floor(diffMs / 1000)
  if (secs < 60) return `Updated ${secs}s ago`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `Updated ${mins}m ago`
  return `Updated ${Math.floor(mins / 60)}h ago`
}

/** Longer reset label for the tooltip, showing both countdown and exact time/date. */
function formatResetLabel(resetAt: string): string {
  if (!resetAt) return ''
  const resetDate = new Date(resetAt)
  const diffMs = resetDate.getTime() - Date.now()
  if (diffMs <= 0) return 'Resets now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  const time = resetDate.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
  if (hours < 24) return `Resets in ${hours}h · ${time}`
  const days = Math.ceil(diffMs / (1000 * 60 * 60 * 24))
  const date = resetDate.toLocaleDateString([], { month: 'short', day: 'numeric' })
  return `Resets in ${days}d · ${date}, ${time}`
}

function barColor(pct: number): string {
  if (pct > 95) return 'bg-red-500'
  if (pct > 80) return 'bg-amber-500'
  return 'bg-blue-500'
}

/** Small inline progress bar for the compact pill. */
function MiniBar({ percentage }: { percentage: number }) {
  return (
    <span className="inline-flex w-10 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
      <span
        className={`h-full rounded-full ${barColor(percentage)}`}
        style={{ width: `${Math.min(100, percentage)}%` }}
      />
    </span>
  )
}

/** Full-width progress bar for the tooltip. */
function ProgressBar({ percentage }: { percentage: number }) {
  return (
    <div className="w-full h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
      <div
        className={`h-full rounded-full transition-all ${barColor(percentage)}`}
        style={{ width: `${Math.min(100, percentage)}%` }}
      />
    </div>
  )
}

/** A single tier row inside the tooltip popover. */
function TierRow({ tier }: { tier: UsageTier }) {
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-4">
        <span className="font-medium text-gray-700 dark:text-gray-300">{tier.label}</span>
        <span className="tabular-nums text-gray-500 dark:text-gray-400">
          {Math.round(tier.percentage)}%
        </span>
      </div>
      <ProgressBar percentage={tier.percentage} />
      <div className="text-xs text-gray-500 dark:text-gray-400">
        {tier.spent && <span>{tier.spent} &middot; </span>}
        {formatResetLabel(tier.resetAt)}
      </div>
    </div>
  )
}

/** Find the session tier, or fall back to the first tier. */
function getSessionTier(tiers: UsageTier[]): UsageTier | undefined {
  return tiers.find((t) => t.id === 'session') ?? tiers[0]
}

/** Convert Unix seconds to ISO-8601 string for tier resetAt. */
function unixSecsToIso(secs: number): string {
  return new Date(secs * 1000).toISOString()
}

/**
 * Overlay real-time statusline rate limit data onto API-polled tiers.
 * Statusline is the primary source for percentage; API tiers provide
 * labels, spent amounts, and tiers statusline doesn't cover (extra).
 */
function applyStatuslineOverrides(
  apiTiers: UsageTier[],
  statusline: StatuslineRateLimit | null | undefined,
): UsageTier[] {
  if (!statusline) return apiTiers

  const tiers = apiTiers.map((tier) => {
    // 5-hour session tier
    if (tier.id === 'session') {
      return {
        ...tier,
        percentage: statusline.pct5h,
        resetAt: unixSecsToIso(statusline.reset5h),
      }
    }
    // 7-day tier (weekly)
    if ((tier.id === 'weekly' || tier.id === 'weekly_sonnet') && statusline.pct7d != null) {
      return {
        ...tier,
        percentage: statusline.pct7d,
        resetAt: statusline.reset7d != null ? unixSecsToIso(statusline.reset7d) : tier.resetAt,
      }
    }
    return tier
  })

  // If API returned no tiers but statusline has data, synthesize a session tier
  if (tiers.length === 0) {
    tiers.push({
      id: 'session',
      label: 'Session (5hr)',
      percentage: statusline.pct5h,
      resetAt: unixSecsToIso(statusline.reset5h),
    })
  }

  return tiers
}

/** Try to extract a human-readable message from the backend error string.
 *  Backend format: `"API error 429 Too Many Requests: {\"error\":{\"message\":\"...\"}}"` */
function parseApiError(raw: string): { status: string; message: string } {
  // Try to extract the JSON payload after the colon
  const jsonStart = raw.indexOf('{')
  if (jsonStart !== -1) {
    try {
      const parsed = JSON.parse(raw.slice(jsonStart))
      const msg = parsed?.error?.message ?? parsed?.message
      if (msg) {
        const statusMatch = raw.match(/^API error (\d+ [^:]+)/)
        return { status: statusMatch?.[1] ?? 'Error', message: msg }
      }
    } catch {
      // Fall through
    }
  }
  return { status: 'Error', message: raw }
}

/** Returns true if orgName is just "<email>'s Organization" — redundant info. */
function isRedundantOrgName(orgName: string, email: string | null): boolean {
  if (!email) return false
  return (
    orgName.toLowerCase().includes(email.split('@')[0].toLowerCase()) &&
    orgName.toLowerCase().endsWith("'s organization")
  )
}

/** Real-time rate limit data derived from statusline SSE events. */
export interface StatuslineRateLimit {
  /** 5-hour session usage percentage (0–100). */
  pct5h: number
  /** 5-hour reset timestamp (Unix seconds). */
  reset5h: number
  /** 7-day usage percentage (0–100), if available. */
  pct7d?: number
  /** 7-day reset timestamp (Unix seconds), if available. */
  reset7d?: number
}

interface OAuthUsagePillProps {
  /** Live rate limit from statusline — overlays onto API-polled tiers for real-time updates. */
  statuslineRateLimit?: StatuslineRateLimit | null
}

export function OAuthUsagePill({ statuslineRateLimit }: OAuthUsagePillProps) {
  const { data, isLoading, error, dataUpdatedAt, forceRefresh } = useOAuthUsage()
  const [tooltipOpen, setTooltipOpen] = useState(false)
  const { data: identity } = useAuthIdentity(tooltipOpen)

  if (isLoading) {
    return <span className="text-xs text-gray-400 dark:text-gray-500">Loading usage...</span>
  }

  if (error) {
    return (
      <span
        className="text-xs text-gray-400 dark:text-gray-500"
        title={error instanceof Error ? error.message : 'Unknown error'}
      >
        Usage unavailable
      </span>
    )
  }

  if (!data || !data.hasAuth) {
    return null
  }

  if (data.error) {
    const parsed = parseApiError(data.error)
    return (
      <Tooltip.Provider delayDuration={300}>
        <Tooltip.Root>
          <Tooltip.Trigger asChild>
            <span className="inline-flex items-center gap-1.5 text-xs text-gray-400 dark:text-gray-500 cursor-default">
              <span className="inline-block h-2 w-2 rounded-full bg-amber-500" />
              Usage unavailable
            </span>
          </Tooltip.Trigger>
          <Tooltip.Portal>
            <Tooltip.Content
              side="bottom"
              align="end"
              sideOffset={8}
              className="z-50 w-72 rounded-lg px-4 py-3 bg-white dark:bg-gray-800 text-xs shadow-xl border border-gray-200 dark:border-gray-700 animate-in fade-in-0 zoom-in-95"
            >
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">
                    Usage
                  </span>
                  {data.plan && (
                    <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
                      {data.plan}
                    </span>
                  )}
                </div>
                <div className="rounded bg-amber-500/10 px-2.5 py-2 text-amber-600 dark:text-amber-400">
                  <div className="font-medium">{parsed.status}</div>
                  <div className="mt-0.5 text-xs text-amber-500 dark:text-amber-500/80">
                    {parsed.message}
                  </div>
                </div>
              </div>
              <Tooltip.Arrow className="fill-white dark:fill-gray-800" />
            </Tooltip.Content>
          </Tooltip.Portal>
        </Tooltip.Root>
      </Tooltip.Provider>
    )
  }

  if (data.tiers.length === 0 && !statuslineRateLimit) {
    return null
  }

  // Apply real-time statusline overrides onto API-polled tiers
  const effectiveTiers = applyStatuslineOverrides(data.tiers, statuslineRateLimit)

  const sessionTier = getSessionTier(effectiveTiers)
  if (!sessionTier) return null

  const isLive = statuslineRateLimit != null

  return (
    <Tooltip.Provider delayDuration={300}>
      <Tooltip.Root
        onOpenChange={(open) => {
          setTooltipOpen(open)
        }}
      >
        <Tooltip.Trigger asChild>
          <span className="inline-flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 font-mono tabular-nums cursor-default">
            {isLive && (
              <span className="relative inline-flex w-1.5 h-1.5 flex-shrink-0">
                <span className="absolute inset-0 rounded-full bg-green-400/60 motion-safe:animate-ping" />
                <span className="relative inline-block w-1.5 h-1.5 rounded-full bg-green-500" />
              </span>
            )}
            <MiniBar percentage={sessionTier.percentage} />
            <span>{Math.round(sessionTier.percentage)}%</span>
            <span className="text-gray-400 dark:text-gray-500">&middot;</span>
            <span>{formatReset(sessionTier.resetAt)}</span>
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            align="end"
            sideOffset={8}
            className="z-50 w-72 rounded-lg px-4 py-3 bg-white dark:bg-gray-800 text-xs shadow-xl border border-gray-200 dark:border-gray-700 animate-in fade-in-0 zoom-in-95"
          >
            {/* Header */}
            <div className="mb-3 pb-2 border-b border-gray-200 dark:border-gray-700">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">
                  Usage
                </span>
                {data.plan && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
                    {data.plan}
                  </span>
                )}
              </div>
              {identity?.hasAuth && identity.email && (
                <div className="mt-1.5 space-y-0.5">
                  <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
                    {identity.email}
                  </div>
                  {identity.orgName && !isRedundantOrgName(identity.orgName, identity.email) && (
                    <div className="text-xs text-gray-400 dark:text-gray-500 truncate">
                      {identity.orgName}
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* Tier rows */}
            <div className="space-y-3">
              {effectiveTiers.map((tier) => (
                <TierRow key={tier.id} tier={tier} />
              ))}
            </div>

            {/* Last refreshed + force refresh */}
            {dataUpdatedAt > 0 && (
              <div className="mt-3 pt-2 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
                <span className="text-xs text-gray-400 dark:text-gray-500">
                  {forceRefresh.isError
                    ? (forceRefresh.error?.message ?? 'Try again later')
                    : formatUpdatedAgo(dataUpdatedAt)}
                </span>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation()
                    forceRefresh.mutate()
                  }}
                  disabled={forceRefresh.isPending}
                  className="p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 disabled:opacity-40 transition-colors"
                  title="Refresh usage"
                >
                  <RefreshCw
                    className={`h-3 w-3 ${forceRefresh.isPending ? 'animate-spin' : ''}`}
                  />
                </button>
              </div>
            )}

            <Tooltip.Arrow className="fill-white dark:fill-gray-800" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
