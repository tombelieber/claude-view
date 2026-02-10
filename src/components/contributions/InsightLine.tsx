import { Info, CheckCircle, AlertTriangle, Lightbulb } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { Insight, InsightKind } from '../../types/generated'

interface InsightLineProps {
  insight: Insight
  className?: string
}

const INSIGHT_ICONS: Record<InsightKind, typeof Info> = {
  info: Info,
  success: CheckCircle,
  warning: AlertTriangle,
  tip: Lightbulb,
}

const INSIGHT_COLORS: Record<InsightKind, string> = {
  info: 'text-blue-600 dark:text-blue-400',
  success: 'text-green-600 dark:text-green-400',
  warning: 'text-amber-600 dark:text-amber-400',
  tip: 'text-purple-600 dark:text-purple-400',
}

const INSIGHT_BG: Record<InsightKind, string> = {
  info: 'bg-blue-50 dark:bg-blue-900/20',
  success: 'bg-green-50 dark:bg-green-900/20',
  warning: 'bg-amber-50 dark:bg-amber-900/20',
  tip: 'bg-purple-50 dark:bg-purple-900/20',
}

/**
 * InsightLine renders a plain-English insight with an icon.
 *
 * Used throughout the contributions page to explain metrics.
 */
export function InsightLine({ insight, className }: InsightLineProps) {
  const Icon = INSIGHT_ICONS[insight.kind]
  const iconColor = INSIGHT_COLORS[insight.kind]
  const bgColor = INSIGHT_BG[insight.kind]

  return (
    <div
      className={cn(
        'flex items-start gap-2 px-3 py-2 rounded-lg text-sm',
        bgColor,
        className
      )}
      role="status"
      aria-label={`${insight.kind}: ${insight.text}`}
    >
      <Icon
        className={cn('w-4 h-4 flex-shrink-0 mt-0.5', iconColor)}
        aria-hidden="true"
      />
      <p className="text-gray-700 dark:text-gray-300">{insight.text}</p>
    </div>
  )
}

/**
 * Compact insight line for inline use within cards.
 */
export function InsightLineCompact({ insight, className }: InsightLineProps) {
  const Icon = INSIGHT_ICONS[insight.kind]
  const iconColor = INSIGHT_COLORS[insight.kind]

  return (
    <div
      className={cn(
        'flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400',
        className
      )}
      aria-label={insight.text}
    >
      <Icon className={cn('w-3 h-3 flex-shrink-0', iconColor)} aria-hidden="true" />
      <span className="truncate" title={insight.text}>{insight.text}</span>
    </div>
  )
}
