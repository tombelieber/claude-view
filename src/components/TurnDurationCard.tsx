import { Clock } from 'lucide-react'
import { cn } from '../lib/utils'

interface TurnDurationCardProps {
  durationMs?: number
  startTime?: string
  endTime?: string
}

export function TurnDurationCard({ durationMs, startTime, endTime }: TurnDurationCardProps) {
  return (
    <div
      className={cn(
        'flex items-center gap-2 my-2 px-3 py-2 rounded-lg',
        'border border-amber-300 bg-amber-50'
      )}
      role="status"
    >
      <Clock className="w-4 h-4 text-amber-600 flex-shrink-0" aria-hidden="true" />
      <span className="text-sm font-medium text-amber-800">
        Turn completed in {durationMs !== undefined ? `${durationMs}ms` : 'unknown duration'}
      </span>
      {startTime && endTime && (
        <span className="text-xs text-amber-600 ml-auto">
          {startTime} — {endTime}
        </span>
      )}
    </div>
  )
}
