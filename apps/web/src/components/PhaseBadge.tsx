import type { SessionPhase } from '@claude-view/shared/types/generated/SessionPhase'
import { cn } from '../lib/utils'

interface PhaseConfig {
  label: string
  emoji: string
  bg: string
  text: string
  border: string
}

const PHASE_CONFIG: Record<Exclude<SessionPhase, 'working'>, PhaseConfig> = {
  thinking: {
    label: 'Thinking',
    emoji: '💭',
    bg: 'bg-purple-100/60 dark:bg-purple-900/25',
    text: 'text-purple-700 dark:text-purple-400',
    border: 'border-purple-300/30 dark:border-purple-800/40',
  },
  planning: {
    label: 'Planning',
    emoji: '📋',
    bg: 'bg-blue-100/60 dark:bg-blue-900/25',
    text: 'text-blue-700 dark:text-blue-400',
    border: 'border-blue-300/30 dark:border-blue-800/40',
  },
  building: {
    label: 'Building',
    emoji: '🔨',
    bg: 'bg-orange-100/60 dark:bg-orange-900/25',
    text: 'text-orange-700 dark:text-orange-400',
    border: 'border-orange-300/30 dark:border-orange-800/40',
  },
  testing: {
    label: 'Testing',
    emoji: '🧪',
    bg: 'bg-green-100/60 dark:bg-green-900/25',
    text: 'text-green-700 dark:text-green-400',
    border: 'border-green-300/30 dark:border-green-800/40',
  },
  reviewing: {
    label: 'Reviewing',
    emoji: '🔍',
    bg: 'bg-cyan-100/60 dark:bg-cyan-900/25',
    text: 'text-cyan-700 dark:text-cyan-400',
    border: 'border-cyan-300/30 dark:border-cyan-800/40',
  },
  shipping: {
    label: 'Shipping',
    emoji: '🚀',
    bg: 'bg-red-100/60 dark:bg-red-900/25',
    text: 'text-red-700 dark:text-red-400',
    border: 'border-red-300/30 dark:border-red-800/40',
  },
}

interface PhaseBadgeProps {
  phase: SessionPhase | null | undefined
  scope?: string | null
  className?: string
}

/**
 * Renders the current SDLC phase as a compact badge.
 * Only shown when the classifier is confident (phase != 'working').
 * Confidence gating happens server-side — if we get a non-working phase, show it.
 */
export function PhaseBadge({ phase, scope, className }: PhaseBadgeProps) {
  if (!phase || phase === 'working') return null

  const config = PHASE_CONFIG[phase]
  if (!config) return null

  return (
    <span
      className={cn(
        'inline-flex items-center gap-0.5 px-1 py-px text-[10px] font-medium rounded border leading-3.5',
        config.bg,
        config.text,
        config.border,
        className,
      )}
    >
      <span>{config.emoji}</span>
      <span>{config.label}</span>
      {scope && (
        <span className="text-muted-foreground ml-1.5">
          · {scope.length > 30 ? `${scope.slice(0, 30)}…` : scope}
        </span>
      )}
    </span>
  )
}
