import * as Tooltip from '@radix-ui/react-tooltip'
import { useOAuthUsage, type UsageTier } from '../../hooks/use-oauth-usage'

/** Human-readable reset countdown from an ISO date. */
function formatReset(resetAt: string): string {
  if (!resetAt) return '--'
  const diffMs = new Date(resetAt).getTime() - Date.now()
  if (diffMs <= 0) return 'now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  if (hours < 24) return `${hours}h`
  return `${Math.ceil(diffMs / (1000 * 60 * 60 * 24))}d`
}

/** Longer reset label for the tooltip. */
function formatResetLabel(resetAt: string): string {
  if (!resetAt) return ''
  const diffMs = new Date(resetAt).getTime() - Date.now()
  if (diffMs <= 0) return 'Resets now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  if (hours < 24) return `Resets in ${hours}h`
  const days = Math.ceil(diffMs / (1000 * 60 * 60 * 24))
  return `Resets in ${days}d`
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
      <div className="text-[10px] text-gray-500 dark:text-gray-400">
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

export function OAuthUsagePill() {
  const { data, isLoading, error, refetch } = useOAuthUsage()

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

  if (!data || !data.hasAuth || data.tiers.length === 0) {
    return null
  }

  const sessionTier = getSessionTier(data.tiers)
  if (!sessionTier) return null

  return (
    <Tooltip.Provider delayDuration={300}>
      <Tooltip.Root onOpenChange={(open) => { if (open) refetch() }}>
        <Tooltip.Trigger asChild>
          <span className="inline-flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 font-mono tabular-nums cursor-default">
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
            <div className="flex items-center justify-between mb-3 pb-2 border-b border-gray-200 dark:border-gray-700">
              <span className="text-[11px] font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">
                Usage
              </span>
              {data.plan && (
                <span className="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
                  {data.plan}
                </span>
              )}
            </div>

            {/* Tier rows */}
            <div className="space-y-3">
              {data.tiers.map((tier) => (
                <TierRow key={tier.id} tier={tier} />
              ))}
            </div>

            <Tooltip.Arrow className="fill-white dark:fill-gray-800" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
