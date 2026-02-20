import { cn } from '../../lib/utils'

const AUTOCOMPACT_THRESHOLD_PCT = 80

interface ContextBarProps {
  percent: number
}

function getFillColor(percent: number): string {
  if (percent >= 90) return 'bg-red-500'
  if (percent >= 75) return 'bg-amber-500'
  return 'bg-sky-500'
}

export function ContextBar({ percent }: ContextBarProps) {
  const clamped = Math.max(0, Math.min(100, percent))

  return (
    <div className="flex items-center gap-1.5">
      <div className="relative w-[40px] h-1 rounded-full bg-gray-200 dark:bg-gray-800 flex-shrink-0">
        <div className="h-full rounded-full overflow-hidden">
          <div
            className={cn('h-full rounded-full transition-all', getFillColor(clamped))}
            style={{ width: `${clamped}%` }}
          />
        </div>
        <div
          className={`absolute top-[-1px] bottom-[-1px] w-[1px] rounded-full ${
            clamped >= AUTOCOMPACT_THRESHOLD_PCT
              ? 'bg-white opacity-90'
              : 'bg-gray-400 dark:bg-gray-600 opacity-40'
          }`}
          style={{ left: `${AUTOCOMPACT_THRESHOLD_PCT}%` }}
          title="~auto-compact threshold"
        />
      </div>
      <span className="text-[10px] tabular-nums text-gray-500 dark:text-gray-400">{clamped}%</span>
    </div>
  )
}
