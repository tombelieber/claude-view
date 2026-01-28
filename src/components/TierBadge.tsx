import { cn } from '../lib/utils'

export interface TierBadgeProps {
  /** Git correlation tier (1 = high confidence, 2 = medium confidence) */
  tier: number
  /** Optional className for additional styling */
  className?: string
}

/**
 * TierBadge displays the git correlation confidence level.
 *
 * - Tier 1: High confidence (commit skill detected) - blue badge
 * - Tier 2: Medium confidence (during session) - gray badge
 */
export function TierBadge({ tier, className }: TierBadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center px-1.5 py-0.5 text-[10px] font-medium rounded',
        tier === 1
          ? 'bg-blue-100 text-blue-700'
          : 'bg-gray-100 text-gray-600',
        className
      )}
      title={tier === 1 ? 'High confidence (commit skill)' : 'Medium confidence (during session)'}
    >
      T{tier}
    </span>
  )
}
