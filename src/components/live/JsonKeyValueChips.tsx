import * as Tooltip from '@radix-ui/react-tooltip'

interface JsonKeyValueChipsProps {
  data: Record<string, unknown>
  maxChips?: number
  maxValueLen?: number
  onExpand: () => void
}

function formatChipValue(value: unknown, maxLen: number): string {
  if (typeof value === 'string') {
    return value.length > maxLen ? value.slice(0, maxLen) + '\u2026' : value
  }
  if (typeof value === 'number' || typeof value === 'boolean') {
    return String(value)
  }
  if (value === null) return 'null'
  if (Array.isArray(value)) return `[${value.length}]`
  if (typeof value === 'object') return '{...}'
  return String(value)
}

function formatFullValue(value: unknown): string {
  if (typeof value === 'string') return value
  if (value === null || value === undefined) return String(value)
  return JSON.stringify(value, null, 2)
}

export function JsonKeyValueChips({
  data,
  maxChips = 3,
  maxValueLen = 40,
  onExpand,
}: JsonKeyValueChipsProps) {
  const entries = Object.entries(data)
  const visible = entries.slice(0, maxChips)
  const remaining = entries.length - maxChips

  return (
    <Tooltip.Provider delayDuration={200}>
    <span className="inline-flex items-center gap-1 flex-wrap">
      {visible.map(([key, value]) => {
        const shortVal = formatChipValue(value, maxValueLen)
        const fullVal = formatFullValue(value)
        const needsTooltip = fullVal !== shortVal

        const chip = (
          <span
            key={key}
            className="inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-[10px] font-mono max-w-[200px]"
          >
            <span className="text-sky-600 dark:text-sky-400 flex-shrink-0">{key}:</span>
            <span className="text-gray-500 dark:text-gray-400 truncate">{shortVal}</span>
          </span>
        )

        if (!needsTooltip) return chip

        return (
          <Tooltip.Root key={key} delayDuration={200}>
            <Tooltip.Trigger asChild>{chip}</Tooltip.Trigger>
            <Tooltip.Portal>
              <Tooltip.Content
                side="bottom"
                align="start"
                className="z-50 max-w-sm px-2 py-1.5 rounded bg-gray-900 dark:bg-gray-100 text-gray-100 dark:text-gray-900 text-[10px] font-mono whitespace-pre-wrap break-all shadow-lg"
                sideOffset={4}
              >
                <span className="text-sky-300 dark:text-sky-600">{key}:</span>{' '}
                {fullVal}
                <Tooltip.Arrow className="fill-gray-900 dark:fill-gray-100" />
              </Tooltip.Content>
            </Tooltip.Portal>
          </Tooltip.Root>
        )
      })}
      {remaining > 0 && (
        <button
          onClick={onExpand}
          className="inline-flex items-center px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-[10px] font-mono text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
        >
          +{remaining} more
        </button>
      )}
    </span>
    </Tooltip.Provider>
  )
}
