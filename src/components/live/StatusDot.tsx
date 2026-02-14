import { cn } from '../../lib/utils'

interface StatusDotProps {
  status: 'working' | 'waiting' | 'idle' | 'done'
  size?: 'sm' | 'md'
  pulse?: boolean
}

const STATUS_COLORS: Record<StatusDotProps['status'], string> = {
  working: 'bg-green-500',
  waiting: 'bg-amber-500',
  idle: 'bg-gray-500',
  done: 'bg-blue-500',
}

const SIZE_CLASSES: Record<NonNullable<StatusDotProps['size']>, string> = {
  sm: 'h-2 w-2',
  md: 'h-2.5 w-2.5',
}

export function StatusDot({ status, size = 'sm', pulse = false }: StatusDotProps) {
  const showPulse = pulse && status === 'working'

  return (
    <span className="relative inline-flex">
      {showPulse && (
        <span
          className={cn(
            'absolute inline-flex rounded-full opacity-75 animate-ping',
            SIZE_CLASSES[size],
            STATUS_COLORS[status]
          )}
        />
      )}
      <span
        className={cn(
          'relative inline-flex rounded-full',
          SIZE_CLASSES[size],
          STATUS_COLORS[status]
        )}
      />
    </span>
  )
}
