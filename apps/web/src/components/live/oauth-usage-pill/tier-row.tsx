import type { UsageTier } from '../../../hooks/use-oauth-usage'
import { formatResetLabel } from './format'

function barColor(pct: number): string {
  if (pct > 95) return 'bg-red-500'
  if (pct > 80) return 'bg-amber-500'
  return 'bg-blue-500'
}

/** Small inline progress bar for the compact pill. */
export function MiniBar({ percentage }: { percentage: number }) {
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
export function TierRow({ tier }: { tier: UsageTier }) {
  const isExtra = tier.kind === 'extra'
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-4">
        <span className="font-medium text-gray-700 dark:text-gray-300 truncate">{tier.label}</span>
        <span className="tabular-nums text-gray-500 dark:text-gray-400 flex-shrink-0">
          {Math.round(tier.percentage)}%
        </span>
      </div>
      <ProgressBar percentage={tier.percentage} />
      <div className="text-xs text-gray-500 dark:text-gray-400 truncate">
        {tier.spent && <span>{tier.spent}</span>}
        {tier.spent && tier.currency && tier.currency !== 'USD' && (
          <span className="ml-1 text-gray-400 dark:text-gray-500">· {tier.currency}</span>
        )}
        {tier.spent && tier.resetAt && <span> · </span>}
        {!isExtra && formatResetLabel(tier.resetAt)}
      </div>
    </div>
  )
}
