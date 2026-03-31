import type { PhaseFreshness } from '@claude-view/shared/types/generated/PhaseFreshness'
import type { SessionPhase } from '@claude-view/shared/types/generated/SessionPhase'
import * as Tooltip from '@radix-ui/react-tooltip'
import { Cpu } from 'lucide-react'
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
  freshness?: PhaseFreshness
  className?: string
}

/**
 * Renders the current SDLC phase as a compact badge.
 * Only shown when the classifier is confident (phase != 'working').
 *
 * Visual states:
 * - `fresh`   → solid badge, full opacity (Running, confirmed phase)
 * - `pending` → brief pulse (~400ms while classify in-flight)
 * - `settled` → dimmed 75% opacity (NeedsYou, phase frozen)
 */
export function PhaseBadge({ phase, scope, freshness, className }: PhaseBadgeProps) {
  if (!phase || phase === 'working') return null

  const config = PHASE_CONFIG[phase]
  if (!config) return null

  const isPending = freshness === 'pending'
  const isSettled = freshness === 'settled'

  const badge = (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded border transition-opacity duration-300',
        config.bg,
        config.text,
        config.border,
        isPending && 'animate-pulse',
        isSettled && 'opacity-75',
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

  const freshnessLabel = isPending
    ? 'Classifying…'
    : isSettled
      ? 'Session idle — phase frozen'
      : 'Live classification'

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>{badge}</Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs"
            sideOffset={5}
          >
            <p className="font-medium text-gray-900 dark:text-gray-100">
              {config.emoji} {config.label} phase
            </p>
            {scope && <p className="text-gray-500 dark:text-gray-400 mt-0.5">{scope}</p>}
            <p className="text-gray-400 dark:text-gray-500 mt-1">
              <Cpu className="inline size-3 mr-0.5 -mt-px" />
              oMLX · Qwen3.5 — {freshnessLabel}
            </p>
            <Tooltip.Arrow className="fill-gray-200 dark:fill-gray-700" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}

/**
 * Skeleton shown while oMLX + Qwen3.5 is classifying the session phase.
 * Displays only for autonomous sessions before the first stabilized result.
 */
export function PhaseBadgeSkeleton({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded border',
        'bg-gray-100/60 dark:bg-gray-800/40',
        'text-gray-400 dark:text-gray-500',
        'border-gray-200/50 dark:border-gray-700/40',
        'animate-pulse',
        className,
      )}
    >
      <Cpu className="size-2.5" />
      <span>oMLX · Qwen3.5</span>
      <span className="inline-flex gap-px ml-0.5">
        <span className="size-1 rounded-full bg-current animate-bounce [animation-delay:0ms]" />
        <span className="size-1 rounded-full bg-current animate-bounce [animation-delay:150ms]" />
        <span className="size-1 rounded-full bg-current animate-bounce [animation-delay:300ms]" />
      </span>
    </span>
  )
}
