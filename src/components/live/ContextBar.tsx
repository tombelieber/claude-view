import { cn } from '../../lib/utils'

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
      <div className="w-[40px] h-1 rounded-full bg-gray-200 dark:bg-gray-800 overflow-hidden flex-shrink-0">
        <div
          className={cn('h-full rounded-full transition-all', getFillColor(clamped))}
          style={{ width: `${clamped}%` }}
        />
      </div>
      <span className="text-[10px] tabular-nums text-gray-500 dark:text-gray-400">{clamped}%</span>
    </div>
  )
}
