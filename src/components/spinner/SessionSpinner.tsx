import { useState, useEffect } from 'react'
import { useMediaQuery } from '@/hooks/use-media-query'
import {
  SPINNER_FRAMES,
  formatTokensCompact,
  formatModelShort,
  formatDurationCompact,
} from './constants'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface BaseSpinnerProps {
  model: string | null
}

interface LiveSpinnerProps extends BaseSpinnerProps {
  mode: 'live'
  durationSeconds: number
  inputTokens: number
  outputTokens: number
  isStalled?: boolean
  agentStateGroup?: 'needs_you' | 'autonomous'
  spinnerVerb?: string
  lastActivityAt?: number
  lastTurnTaskSeconds?: number | null
}

interface HistoricalSpinnerProps extends BaseSpinnerProps {
  mode: 'historical'
  pastTenseVerb?: string
  taskTimeSeconds?: number | null
}

export type SessionSpinnerProps = LiveSpinnerProps | HistoricalSpinnerProps

// ---------------------------------------------------------------------------
// Animation constants
// ---------------------------------------------------------------------------

/** Bounce indices into SPINNER_FRAMES for a smooth back-and-forth */
const BOUNCE_SEQUENCE = [0, 1, 2, 3, 4, 5, 4, 3, 2, 1] as const

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function SessionSpinner(props: SessionSpinnerProps) {
  const { mode, model } = props
  const prefersReducedMotion = useMediaQuery('(prefers-reduced-motion: reduce)')

  const [frame, setFrame] = useState<string>(SPINNER_FRAMES[0])

  // Determine live-mode specifics via discriminant narrowing (no `as` casts)
  const agentStateGroup = props.mode === 'live' ? props.agentStateGroup ?? 'autonomous' : 'autonomous'
  const isStalled = props.mode === 'live' ? props.isStalled ?? false : false
  const lastActivityAt = props.mode === 'live' ? props.lastActivityAt ?? 0 : 0

  // ---------------------------------------------------------------------------
  // RAF animation — only runs in live + autonomous mode
  // ---------------------------------------------------------------------------
  useEffect(() => {
    if (mode !== 'live') return
    if (agentStateGroup !== 'autonomous') return
    if (prefersReducedMotion) return

    let rafId: number
    let lastFrameTime = 0
    let frameIdx = 0

    function tick(now: number) {
      if (now - lastFrameTime >= 200) {
        lastFrameTime = now
        frameIdx = (frameIdx + 1) % BOUNCE_SEQUENCE.length
        setFrame(SPINNER_FRAMES[BOUNCE_SEQUENCE[frameIdx]])
      }
      rafId = requestAnimationFrame(tick)
    }
    rafId = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(rafId)
  }, [mode, agentStateGroup, prefersReducedMotion])

  // Countdown tick for needs_you wait-time display (1s resolution)
  const [, setTick] = useState(0)
  useEffect(() => {
    if (agentStateGroup !== 'needs_you') return
    const interval = setInterval(() => setTick(t => t + 1), 1000)
    return () => clearInterval(interval)
  }, [agentStateGroup])

  // ---------------------------------------------------------------------------
  // Historical mode
  // ---------------------------------------------------------------------------
  if (mode === 'historical') {
    const verb = props.pastTenseVerb ?? 'Worked'
    const taskTime = props.taskTimeSeconds
    const formattedTaskTime = taskTime && taskTime > 0 ? formatDurationCompact(taskTime) : null
    const shortModel = formatModelShort(model)

    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-gray-500 dark:text-gray-400">·</span>
        <span className="text-gray-500 dark:text-gray-400">{verb}</span>
        {formattedTaskTime && (
          <span className="text-gray-500 dark:text-gray-400 font-mono tabular-nums">{formattedTaskTime}</span>
        )}
        {shortModel && (
          <>
            <span className="text-gray-500 dark:text-gray-400"> · </span>
            <span className="text-gray-500 dark:text-gray-400 font-mono tabular-nums">{shortModel}</span>
          </>
        )}
      </span>
    )
  }

  // ---------------------------------------------------------------------------
  // Live mode
  // ---------------------------------------------------------------------------
  const {
    durationSeconds,
    inputTokens,
    outputTokens,
    spinnerVerb,
  } = props

  const shortModel = formatModelShort(model)

  // needs_you -> show completed task time if available, else "Awaiting input"
  // + cache countdown
  if (agentStateGroup === 'needs_you') {
    const lastTurnTaskSeconds = props.mode === 'live' ? props.lastTurnTaskSeconds : null
    const hasBakedTime = lastTurnTaskSeconds != null && lastTurnTaskSeconds > 0

    const CACHE_TTL = 300
    const lastActivity = lastActivityAt
    const elapsed = lastActivity > 0 ? Math.floor(Date.now() / 1000) - lastActivity : CACHE_TTL
    const remaining = Math.max(0, CACHE_TTL - elapsed)

    const countdownText = remaining > 0
      ? `${Math.floor(remaining / 60)}:${String(remaining % 60).padStart(2, '0')}`
      : 'cache cold'

    const countdownColor = remaining <= 0
      ? 'text-gray-400'
      : remaining < 60
        ? 'text-red-500'
        : remaining < 180
          ? 'text-amber-500'
          : 'text-green-500'

    return (
      <span className="flex items-center gap-1.5 text-xs">
        {hasBakedTime ? (
          <>
            <span className="w-3 text-center inline-block text-amber-500">✻</span>
            <span className="text-gray-500 dark:text-gray-400">Baked {formatDurationCompact(lastTurnTaskSeconds)}</span>
          </>
        ) : (
          <>
            <span className="w-3 text-center inline-block text-amber-500">●</span>
            <span className="text-gray-500 dark:text-gray-400">Awaiting input</span>
          </>
        )}
        <span className={`font-mono tabular-nums ${countdownColor}`}>· {countdownText}</span>
      </span>
    )
  }

  // autonomous (default) -> animated spinner + metrics
  const verb = spinnerVerb ?? 'Working'
  const tokenCount = inputTokens + outputTokens
  const arrow = inputTokens >= outputTokens ? '↑' : '↓'
  const formattedTokens = formatTokensCompact(tokenCount)
  const formattedDuration = formatDurationCompact(durationSeconds)

  // Spinner character: animated frame, or static dot for reduced motion
  const spinnerChar = prefersReducedMotion ? '·' : frame

  // Color: red when stalled, green otherwise, with 2s CSS transition
  const spinnerColorClass = isStalled ? 'text-red-500' : 'text-emerald-500'

  return (
    <span className="flex items-center gap-1.5 text-xs">
      <span
        className={`w-3 text-center inline-block transition-colors duration-[2000ms] ${spinnerColorClass}`}
      >
        {spinnerChar}
      </span>
      <span className="text-gray-500 dark:text-gray-400">{verb}…</span>
      <span className="text-gray-500 dark:text-gray-400 font-mono tabular-nums">
        {formattedDuration}
        {' · '}
        {arrow}{formattedTokens}
        {shortModel && (
          <>
            {' · '}
            {shortModel}
          </>
        )}
      </span>
    </span>
  )
}
