import { Clock } from 'lucide-react'

interface TurnDurationCardProps {
  durationMs?: number
  startTime?: string
  endTime?: string
}

export function TurnDurationCard({ durationMs, startTime, endTime }: TurnDurationCardProps) {
  return (
    <div
      className="py-0.5 border-l-2 border-l-amber-400 pl-1 my-1"
      role="status"
    >
      <div className="flex items-center gap-1.5">
        <Clock className="w-3 h-3 text-amber-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
          Turn completed in {durationMs !== undefined ? `${durationMs}ms` : 'unknown duration'}
        </span>
        {startTime && endTime && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-500 ml-auto">
            {startTime} — {endTime}
          </span>
        )}
      </div>
    </div>
  )
}
