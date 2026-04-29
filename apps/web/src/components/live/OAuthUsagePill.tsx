import * as Tooltip from '@radix-ui/react-tooltip'
import { useState } from 'react'
import { useAuthIdentity } from '../../hooks/use-auth-identity'
import { useOAuthUsage } from '../../hooks/use-oauth-usage'
import { formatReset, parseApiError } from './oauth-usage-pill/format'
import {
  applyStatuslineOverrides,
  getSessionTier,
  withInferredKind,
} from './oauth-usage-pill/statusline-overlay'
import { MiniBar } from './oauth-usage-pill/tier-row'
import { UsageTooltipContent } from './oauth-usage-pill/tooltip-content'
import type { StatuslineRateLimit } from './oauth-usage-pill/types'

// Re-exported so existing imports (`import { StatuslineRateLimit } from './OAuthUsagePill'`)
// keep working after the decomposition.
export type { StatuslineRateLimit } from './oauth-usage-pill/types'

interface OAuthUsagePillProps {
  /** Live rate-limit overlay from the statusline SSE — overrides API-polled
   *  values for the 5h + 7d windows so the pill updates within seconds. */
  statuslineRateLimit?: StatuslineRateLimit | null
}

const TOOLTIP_CONTENT_CLASS =
  'z-50 w-72 rounded-lg px-4 py-3 bg-white dark:bg-gray-800 text-xs shadow-xl border border-gray-200 dark:border-gray-700 animate-in fade-in-0 zoom-in-95'

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
    return <UpstreamErrorPill plan={data.plan ?? null} rawError={data.error} />
  }

  if (data.tiers.length === 0 && !statuslineRateLimit) {
    return null
  }

  // Hydrate `kind` for backward compat with pre-2026-04 backends, then overlay
  // real-time statusline values on top of the polled API tiers.
  const effectiveTiers = applyStatuslineOverrides(withInferredKind(data.tiers), statuslineRateLimit)

  const sessionTier = getSessionTier(effectiveTiers)
  if (!sessionTier) return null

  const isLive = statuslineRateLimit != null

  return (
    <Tooltip.Provider delayDuration={300}>
      <Tooltip.Root onOpenChange={setTooltipOpen}>
        <Tooltip.Trigger asChild>
          <span className="inline-flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 font-mono tabular-nums cursor-default">
            {isLive && <LivePulseDot />}
            <MiniBar percentage={sessionTier.percentage} />
            <span>{Math.round(sessionTier.percentage)}%</span>
            <span className="text-gray-400 dark:text-gray-500">·</span>
            <span>{formatReset(sessionTier.resetAt)}</span>
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            align="end"
            sideOffset={8}
            className={TOOLTIP_CONTENT_CLASS}
          >
            <UsageTooltipContent
              data={data}
              identity={identity}
              effectiveTiers={effectiveTiers}
              dataUpdatedAt={dataUpdatedAt}
              forceRefresh={forceRefresh}
            />
            <Tooltip.Arrow className="fill-white dark:fill-gray-800" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}

function LivePulseDot() {
  return (
    <span className="relative inline-flex w-1.5 h-1.5 flex-shrink-0">
      <span className="absolute inset-0 rounded-full bg-green-400/60 motion-safe:animate-ping" />
      <span className="relative inline-block w-1.5 h-1.5 rounded-full bg-green-500" />
    </span>
  )
}

function UpstreamErrorPill({ plan, rawError }: { plan: string | null; rawError: string }) {
  const parsed = parseApiError(rawError)
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
            className={TOOLTIP_CONTENT_CLASS}
          >
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">
                  Usage
                </span>
                {plan && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
                    {plan}
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
