import { cn } from '../../lib/utils'
import type { AgentStateGroup } from './types'

interface StatusDotProps {
  group: AgentStateGroup
  size?: 'sm' | 'md'
  pulse?: boolean
}

const GROUP_COLORS: Record<AgentStateGroup, string> = {
  needs_you: 'bg-amber-500',
  autonomous: 'bg-green-500',
}

const SIZE_CLASSES: Record<NonNullable<StatusDotProps['size']>, string> = {
  sm: 'h-2 w-2',
  md: 'h-2.5 w-2.5',
}

export function StatusDot({ group, size = 'sm', pulse = false }: StatusDotProps) {
  const showPulse = pulse && group === 'autonomous'

  return (
    <span className="relative inline-flex">
      {showPulse && (
        <span
          className={cn(
            'absolute inline-flex rounded-full opacity-75 animate-ping',
            SIZE_CLASSES[size],
            GROUP_COLORS[group]
          )}
        />
      )}
      <span
        className={cn(
          'relative inline-flex rounded-full',
          SIZE_CLASSES[size],
          GROUP_COLORS[group]
        )}
      />
    </span>
  )
}
