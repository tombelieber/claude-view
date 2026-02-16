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
  agentStateGroup?: 'needs_you' | 'autonomous' | 'delivered'
  spinnerVerb?: string
}

interface HistoricalSpinnerProps extends BaseSpinnerProps {
  mode: 'historical'
  pastTenseVerb?: string
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

  // ---------------------------------------------------------------------------
  // Historical mode
  // ---------------------------------------------------------------------------
  if (mode === 'historical') {
    const verb = props.pastTenseVerb ?? 'Worked'
    const shortModel = formatModelShort(model)

    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-muted-foreground">·</span>
        <span className="text-muted-foreground">{verb}</span>
        {shortModel && (
          <>
            <span className="text-muted-foreground"> · </span>
            <span className="text-muted-foreground font-mono tabular-nums">{shortModel}</span>
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

  // needs_you -> amber dot + "Awaiting input"
  if (agentStateGroup === 'needs_you') {
    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-amber-500">●</span>
        <span className="text-muted-foreground">Awaiting input</span>
      </span>
    )
  }

  // delivered -> green check + "Done"
  if (agentStateGroup === 'delivered') {
    return (
      <span className="flex items-center gap-1.5 text-xs">
        <span className="w-3 text-center inline-block text-green-500">✓</span>
        <span className="text-muted-foreground">Done</span>
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
      <span className="text-muted-foreground">{verb}…</span>
      <span className="text-muted-foreground font-mono tabular-nums">
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
