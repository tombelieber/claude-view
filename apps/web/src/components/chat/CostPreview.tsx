import * as Tooltip from '@radix-ui/react-tooltip'

interface CostPreviewProps {
  cached: number
  uncached: number
}

function formatUsd(value: number): string {
  return `$${value.toFixed(2)}`
}

/**
 * Compact inline cost display: "~$X.XX" with tooltip breakdown.
 */
export function CostPreview({ cached, uncached }: CostPreviewProps) {
  const total = cached + uncached

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <span className="text-xs text-gray-500 dark:text-gray-400 tabular-nums cursor-default">
            ~{formatUsd(total)}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="top"
            sideOffset={4}
            className="z-50 rounded-md px-3 py-1.5 text-xs bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            ~{formatUsd(cached)} cached / ~{formatUsd(uncached)} uncached
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
