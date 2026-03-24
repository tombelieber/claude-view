const GAP_THRESHOLD_S = 30 * 60 // 30 minutes in SECONDS

export function shouldShowSeparator(
  prevTimestamp: number | undefined,
  currTimestamp: number | undefined,
): boolean {
  if (!prevTimestamp || !currTimestamp) return false
  return currTimestamp - prevTimestamp > GAP_THRESHOLD_S
}

export function formatRelativeDate(unixSeconds: number): string {
  const date = new Date(unixSeconds * 1000) // convert seconds -> ms for Date
  return date.toLocaleString('en-US', {
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  })
}

export function TimeSeparator({ timestamp }: { timestamp: number }) {
  return (
    <div className="flex items-center gap-3 py-4 text-xs text-gray-400 dark:text-[#8B949E]">
      <div className="flex-1 border-t border-gray-200 dark:border-[#30363D]" />
      <span>{formatRelativeDate(timestamp)}</span>
      <div className="flex-1 border-t border-gray-200 dark:border-[#30363D]" />
    </div>
  )
}
