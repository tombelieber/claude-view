import { FlaskConical } from 'lucide-react'
import { cn } from '../lib/utils'
import * as Tooltip from '@radix-ui/react-tooltip'

interface ExperimentalBadgeProps {
  /** Compact mode: icon only, no text */
  compact?: boolean
  className?: string
  /** Custom tooltip text */
  tooltip?: string
}

const DEFAULT_TOOLTIP = 'This feature is experimental. Results may not be accurate and could change in future updates.'

/**
 * Small inline "Experimental" pill badge with flask icon.
 * Used to mark AI-powered features that are still being refined.
 */
export function ExperimentalBadge({ compact, className, tooltip }: ExperimentalBadgeProps) {
  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <span
            className={cn(
              'inline-flex items-center gap-1 rounded-full border cursor-default',
              'border-amber-300 dark:border-amber-700 bg-amber-50 dark:bg-amber-950/40',
              'text-amber-700 dark:text-amber-400',
              compact
                ? 'px-1 py-0.5'
                : 'px-1.5 py-0.5 text-[10px] font-medium',
              className,
            )}
          >
            <FlaskConical className={cn(compact ? 'w-2.5 h-2.5' : 'w-3 h-3')} />
            {!compact && <span>Experimental</span>}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            sideOffset={4}
            className="z-50 max-w-xs rounded-md px-3 py-2 text-xs leading-relaxed bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            {tooltip || DEFAULT_TOOLTIP}
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
