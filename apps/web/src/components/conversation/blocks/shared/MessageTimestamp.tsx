import * as Tooltip from '@radix-ui/react-tooltip'

interface MessageTimestampProps {
  /** Unix seconds */
  timestamp: number | undefined
  align?: 'left' | 'right'
}

function formatTime(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000)
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function formatFullTimestamp(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000)
  return date.toLocaleString([], {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  })
}

export function MessageTimestamp({ timestamp, align = 'left' }: MessageTimestampProps) {
  if (!timestamp || timestamp <= 0) return null

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <span
            className={[
              'text-[10px] text-gray-400 dark:text-gray-500 tabular-nums cursor-default',
              align === 'right' ? 'text-right' : 'text-left',
            ].join(' ')}
          >
            {formatTime(timestamp)}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            sideOffset={4}
            className="z-50 rounded-md px-2.5 py-1.5 text-xs bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 shadow-lg animate-in fade-in-0 zoom-in-95"
          >
            {formatFullTimestamp(timestamp)}
            <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
