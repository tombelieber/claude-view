import * as Tooltip from '@radix-ui/react-tooltip'

interface ChatContextGaugeProps {
  percent: number
}

/**
 * Compact context-window usage gauge with color-coded bar and tooltip.
 * Blue < 60%, amber 60-80%, red > 80%.
 */
export function ChatContextGauge({ percent }: ChatContextGaugeProps) {
  const clamped = Math.max(0, Math.min(100, percent))

  const barColor =
    clamped > 80
      ? 'bg-red-500 dark:bg-red-400'
      : clamped >= 60
        ? 'bg-amber-500 dark:bg-amber-400'
        : 'bg-blue-500 dark:bg-blue-400'

  const textColor =
    clamped > 80
      ? 'text-red-600 dark:text-red-400'
      : clamped >= 60
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-gray-500 dark:text-gray-400'

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <div className="flex items-center gap-1.5 cursor-default">
            <div className="w-16 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
              <div
                className={`h-full rounded-full transition-all duration-300 ${barColor}`}
                style={{ width: `${clamped}%` }}
              />
            </div>
            <span className={`text-[10px] font-medium tabular-nums ${textColor}`}>
              {Math.round(clamped)}%
            </span>
          </div>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="top"
            sideOffset={4}
            className="z-50 rounded-md px-3 py-1.5 text-xs bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            Context window: {Math.round(clamped)}% used
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
