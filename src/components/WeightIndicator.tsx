import * as Tooltip from '@radix-ui/react-tooltip'
import { cn } from '../lib/utils'
import { type WeightTier, weightDotClass } from '../lib/session-weight'

interface WeightIndicatorProps {
  tier: WeightTier
  /** Render as inline element (for table cells) vs block (for card header) */
  inline?: boolean
}

const TIERS: { tier: WeightTier; label: string; description: string }[] = [
  { tier: 0, label: 'Tiny', description: '< 3 prompts · < 5K tokens' },
  { tier: 1, label: 'Light', description: '3-10 prompts · 5K-50K tokens' },
  { tier: 2, label: 'Medium', description: '10-25 prompts · 50K-200K tokens' },
  { tier: 3, label: 'Heavy', description: '25-50 prompts · 200K-500K tokens' },
  { tier: 4, label: 'Massive', description: '50+ prompts · 500K+ tokens' },
]

/**
 * Small colored dot with a tooltip showing the session weight legend.
 * The current session's tier is highlighted in the legend.
 */
export function WeightIndicator({ tier, inline }: WeightIndicatorProps) {
  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <span
            className={cn(
              'cursor-default flex-shrink-0',
              inline ? 'inline-flex items-center' : 'flex items-center',
            )}
          >
            <span className={cn('w-2 h-2 rounded-full', weightDotClass(tier))} />
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            align="start"
            sideOffset={6}
            className="z-50 rounded-lg px-3 py-2.5 bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            <p className="text-[11px] font-medium mb-2 text-gray-400 dark:text-gray-500 uppercase tracking-wider">
              Session weight
            </p>
            <div className="flex flex-col gap-1.5">
              {TIERS.map((t) => (
                <div
                  key={t.tier}
                  className={cn(
                    'flex items-center gap-2 rounded px-1.5 py-0.5 -mx-1.5',
                    t.tier === tier && 'bg-white/10 dark:bg-black/10',
                  )}
                >
                  <span className={cn('w-2 h-2 rounded-full flex-shrink-0', weightDotClass(t.tier))} />
                  <span className={cn(
                    'text-[11px] font-medium w-14',
                    t.tier === tier ? 'text-white dark:text-gray-900' : 'text-gray-400 dark:text-gray-500',
                  )}>
                    {t.label}
                  </span>
                  <span className={cn(
                    'text-[10px] tabular-nums',
                    t.tier === tier ? 'text-gray-200 dark:text-gray-700' : 'text-gray-500 dark:text-gray-400',
                  )}>
                    {t.description}
                  </span>
                </div>
              ))}
            </div>
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
