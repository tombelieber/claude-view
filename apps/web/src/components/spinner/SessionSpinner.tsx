import { useState, useEffect } from 'react'
import * as Tooltip from '@radix-ui/react-tooltip'
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
  agentStateLabel?: string
  /** The raw agent state key (e.g. "acting", "compacting"). Used for precise state detection. */
  agentStateKey?: string
  spinnerVerb?: string
  lastCacheHitAt?: number | null
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
  const agentStateLabel = props.mode === 'live' ? props.agentStateLabel : undefined
  const agentStateKey = props.mode === 'live' ? props.agentStateKey : undefined
  const isStalled = props.mode === 'live' ? props.isStalled ?? false : false
  const lastCacheHitAt = props.mode === 'live' ? props.lastCacheHitAt ?? null : null

  const isCompacting = agentStateKey === 'compacting'

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
  // + cache countdown (only if lastCacheHitAt is set)
  if (agentStateGroup === 'needs_you') {
    const lastTurnTaskSeconds = props.mode === 'live' ? props.lastTurnTaskSeconds : null
    const hasBakedTime = lastTurnTaskSeconds != null && lastTurnTaskSeconds > 0

    const CACHE_TTL = 300
    const lastCacheHit = lastCacheHitAt
    const hasCacheHit = lastCacheHit != null && lastCacheHit > 0

    const elapsed = hasCacheHit ? Math.floor(Date.now() / 1000) - lastCacheHit : CACHE_TTL
    const remaining = Math.max(0, CACHE_TTL - elapsed)

    const showCountdown = hasCacheHit

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
        {showCountdown ? (
          <CacheCountdownTooltip remaining={remaining} ttl={CACHE_TTL}>
            <span
              className={`font-mono tabular-nums cursor-default ${countdownColor}`}
              title={remaining > 0 ? 'Cache warm' : 'Cache cold'}
            >
              · {countdownText}
            </span>
          </CacheCountdownTooltip>
        ) : (
          <span className="font-mono tabular-nums cursor-default text-gray-400" title="No cache activity yet">
            · –
          </span>
        )}
      </span>
    )
  }

  // autonomous (default) -> animated spinner + metrics
  const verb = isCompacting ? 'Compacting' : (spinnerVerb ?? 'Working')
  const tokenCount = inputTokens + outputTokens
  const arrow = isCompacting ? '↓' : (inputTokens >= outputTokens ? '↑' : '↓')
  const formattedTokens = formatTokensCompact(tokenCount)
  const formattedDuration = formatDurationCompact(durationSeconds)

  const spinnerChar = prefersReducedMotion 
    ? (isCompacting ? '◈' : '·') 
    : (isCompacting ? '◈' : frame)

  const spinnerColorClass = isStalled 
    ? 'text-red-500' 
    : isCompacting 
      ? 'text-amber-500' 
      : 'text-emerald-500'

  const animationClass = isCompacting && !prefersReducedMotion
    ? 'animate-pulse'
    : ''

  return (
    <span className="flex items-center gap-1.5 text-xs">
      <span
        className={`w-3 text-center inline-block transition-colors duration-[2000ms] ${spinnerColorClass} ${animationClass}`}
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

// ---------------------------------------------------------------------------
// Cache Countdown Tooltip
// ---------------------------------------------------------------------------

function CacheCountdownTooltip({
  remaining,
  ttl,
  children,
}: {
  remaining: number
  ttl: number
  children: React.ReactNode
}) {
  const progress = remaining / ttl
  const isWarm = remaining > 0
  const progressPct = Math.min(progress * 100, 100)

  const barColor = !isWarm
    ? 'bg-gray-400'
    : progress < 0.2
      ? 'bg-red-500'
      : progress < 0.6
        ? 'bg-amber-500'
        : 'bg-green-500'

  const minutes = Math.floor(remaining / 60)
  const seconds = remaining % 60
  const timeDisplay = isWarm
    ? `${minutes}:${String(seconds).padStart(2, '0')}`
    : 'Expired'

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          {children}
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content
            side="bottom"
            sideOffset={6}
            className="z-50 w-64 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-3 text-xs animate-in fade-in-0 zoom-in-95"
            onClick={(e) => e.stopPropagation()}
          >
            {/* Header */}
            <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">
              Prompt Cache
            </div>

            {/* Status + countdown */}
            <div className="flex items-center justify-between text-gray-500 dark:text-gray-400 mb-2">
              <span className={isWarm ? 'text-green-600 dark:text-green-400' : 'text-gray-400 dark:text-gray-500'}>
                {isWarm ? 'Warm' : 'Cold'}
              </span>
              <span className="tabular-nums font-mono">{timeDisplay}</span>
            </div>

            {/* Progress bar */}
            <div className="h-1.5 rounded-full bg-gray-200 dark:bg-gray-800 overflow-hidden mb-3">
              {progressPct > 0 && (
                <div
                  className={`${barColor} h-full transition-all duration-300`}
                  style={{ width: `${progressPct}%` }}
                />
              )}
            </div>

            {/* Breakdown */}
            <div className="space-y-0.5 text-[11px]">
              <div className="text-gray-400 dark:text-gray-500 text-[10px] uppercase tracking-wide mb-1">How it works</div>
              <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
                <span>TTL</span>
                <span className="tabular-nums font-mono">5 min</span>
              </div>
              <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
                <span>Resets on</span>
                <span className="font-mono">each API call</span>
              </div>
              <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
                <span>Cache read</span>
                <span className="font-mono">90% cheaper</span>
              </div>
            </div>

            {/* Explainer */}
            <div className="border-t border-gray-200 dark:border-gray-700 pt-2 mt-2 text-[11px] text-gray-500 dark:text-gray-400 leading-relaxed">
              {isWarm
                ? 'Cache is warm — the next turn reuses cached tokens (faster + cheaper). Respond before it expires to keep the savings.'
                : 'Cache expired — the next turn will re-cache the system prompt and conversation prefix. First turn back costs more.'}
            </div>

            {/* Link */}
            <div className="border-t border-gray-200 dark:border-gray-700 pt-2 mt-2">
              <a
                href="https://platform.claude.com/docs/en/build-with-claude/prompt-caching"
                target="_blank"
                rel="noopener noreferrer"
                className="text-[10px] text-sky-500 dark:text-sky-400 hover:underline"
                onClick={(e) => e.stopPropagation()}
              >
                Learn about prompt caching →
              </a>
            </div>

            <Tooltip.Arrow className="fill-white dark:fill-gray-900" />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
