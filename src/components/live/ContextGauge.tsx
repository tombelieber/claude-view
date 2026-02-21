import { useState, useRef, useEffect, useCallback, useLayoutEffect } from 'react'
import { createPortal } from 'react-dom'
import { Minimize2 } from 'lucide-react'
import type { AgentStateGroup } from './types'

const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  'claude-opus-4': 200_000,
  'claude-sonnet-4': 200_000,
  'claude-haiku-4': 200_000,
  'claude-3': 200_000,
}

function getContextLimit(model: string | null): number {
  if (!model) return 200_000
  for (const [prefix, limit] of Object.entries(MODEL_CONTEXT_LIMITS)) {
    if (model.startsWith(prefix)) return limit
  }
  return 200_000
}

interface ContextGaugeProps {
  /** Current context window fill (last turn's total input tokens). */
  contextWindowTokens: number
  model: string | null
  group: AgentStateGroup
  /** Cumulative token counts for the session (optional — shows breakdown when provided). */
  tokens?: {
    inputTokens: number
    outputTokens: number
    cacheReadTokens: number
    cacheCreationTokens: number
    totalTokens: number
  }
  /** Number of user turns (optional). */
  turnCount?: number
  /** When true, shows breakdown inline (for side panel). When false, shows in hover tooltip (for cards). */
  expanded?: boolean
  /** Current agent state label — shown in compacting overlay. */
  agentLabel?: string
  /** The raw agent state key (e.g. "acting", "compacting"). Used for precise state detection. */
  agentStateKey?: string
}

const formatTokens = (n: number) => {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return String(n)
}

const TOOLTIP_W = 256 // w-64
const MARGIN = 8

// Segment colors for the stacked bar
const SEGMENT_COLORS = {
  system: 'bg-slate-400 dark:bg-slate-500',
  conversation: 'bg-sky-500',
  buffer: 'bg-amber-400 dark:bg-amber-500',
}

/** Approximate context percentage at which Claude Code triggers auto-compaction. */
const AUTOCOMPACT_THRESHOLD_PCT = 80

export function ContextGauge({ contextWindowTokens, model, group, tokens, turnCount, expanded = false, agentLabel, agentStateKey }: ContextGaugeProps) {
  const contextLimit = getContextLimit(model)
  const usedPct = Math.min((contextWindowTokens / contextLimit) * 100, 100)
  const [isOpen, setIsOpen] = useState(false)

  // Compacting state detection — use the state key, not the label text
  // (labels can contain "compacting" when the agent is searching/grepping for that word)
  const isCompacting = agentStateKey === 'compacting'
  const prevStateKeyRef = useRef(agentStateKey)
  const [justCompacted, setJustCompacted] = useState(false)

  useEffect(() => {
    const wasCompacting = prevStateKeyRef.current === 'compacting'
    if (wasCompacting && !isCompacting) {
      setJustCompacted(true)
      const timer = setTimeout(() => setJustCompacted(false), 5_000)
      return () => clearTimeout(timer)
    }
    prevStateKeyRef.current = agentStateKey
  }, [agentStateKey, isCompacting])
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()
  const containerRef = useRef<HTMLDivElement>(null)
  const tooltipRef = useRef<HTMLDivElement>(null)
  const [tooltipStyle, setTooltipStyle] = useState<React.CSSProperties>({})

  const computePlacement = useCallback(() => {
    const container = containerRef.current
    const tooltip = tooltipRef.current
    if (!container || !tooltip) return
    const rect = container.getBoundingClientRect()
    const tipH = tooltip.offsetHeight
    const topBoundary = 56
    const spaceAbove = rect.top - topBoundary
    const spaceBelow = window.innerHeight - rect.bottom
    const spaceRight = window.innerWidth - rect.left

    const above = spaceBelow >= tipH + MARGIN ? false
      : spaceAbove >= tipH + MARGIN ? true
      : spaceBelow >= spaceAbove ? false : true

    const top = above ? rect.top - tipH - MARGIN : rect.bottom + MARGIN
    const left = spaceRight >= TOOLTIP_W + MARGIN
      ? rect.left
      : rect.right - TOOLTIP_W

    setTooltipStyle({
      position: 'fixed',
      top: Math.max(topBoundary, top),
      left: Math.max(MARGIN, left),
      zIndex: 9999,
    })
  }, [])

  useLayoutEffect(() => {
    if (isOpen) computePlacement()
  }, [isOpen, computePlacement])

  const handleMouseEnter = () => {
    if (expanded) return // No tooltip in expanded mode
    clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => setIsOpen(true), 200)
  }

  const handleMouseLeave = () => {
    if (expanded) return
    clearTimeout(timeoutRef.current)
    timeoutRef.current = setTimeout(() => setIsOpen(false), 100)
  }

  const isInactive = group === 'needs_you'
  const barColor = isInactive
    ? 'bg-zinc-500 opacity-50'
    : usedPct > 90
      ? 'bg-red-500'
      : usedPct > 75
        ? 'bg-amber-500'
        : 'bg-sky-500'

  // Estimate breakdown
  const systemEstimate = Math.min(20_000, contextWindowTokens)
  const messagesEstimate = Math.max(0, contextWindowTokens - systemEstimate)
  const autocompactBuffer = Math.round(contextLimit * 0.165)
  const freeSpace = Math.max(0, contextLimit - contextWindowTokens - autocompactBuffer)

  // Segment percentages for stacked bar
  const systemPct = (systemEstimate / contextLimit) * 100
  const msgPct = (messagesEstimate / contextLimit) * 100
  const bufferPct = (autocompactBuffer / contextLimit) * 100

  // Cache efficiency percentage
  const cacheEfficiency = tokens && tokens.totalTokens > 0
    ? Math.round((tokens.cacheReadTokens / tokens.totalTokens) * 100)
    : 0

  // ---- Expanded mode: inline breakdown ----
  if (expanded) {
    return (
      <div className="space-y-3">
        {/* Model + usage header */}
        <div className="flex items-center justify-between text-xs">
          <span className="text-gray-500 dark:text-gray-400 font-mono">{model ?? 'unknown'}</span>
          <span className={`font-mono tabular-nums font-medium ${
            usedPct > 90 ? 'text-red-500' : usedPct > 75 ? 'text-amber-500' : 'text-gray-900 dark:text-gray-100'
          }`}>
            {usedPct.toFixed(1)}% used
          </span>
        </div>

        {/* Compacting status */}
        {isCompacting && (
          <div className="flex items-center gap-1.5 text-xs">
            <Minimize2 className="h-3 w-3 text-blue-500 dark:text-blue-400 motion-safe:animate-pulse" />
            <span className="text-blue-500 dark:text-blue-400">compacting...</span>
          </div>
        )}
        {!isCompacting && justCompacted && (
          <span className="text-xs text-green-500 dark:text-green-400 animate-pulse">
            compacted
          </span>
        )}

        {/* Stacked segmented bar */}
        <div className={`relative h-2.5 rounded-full bg-gray-200 dark:bg-gray-800${isCompacting ? ' motion-safe:animate-pulse' : ''}`}>
          <div className="h-full rounded-full overflow-hidden flex">
            {systemPct > 0 && (
              <div className={`${SEGMENT_COLORS.system} h-full`} style={{ width: `${systemPct}%` }} />
            )}
            {msgPct > 0 && (
              <div className={`${SEGMENT_COLORS.conversation} h-full`} style={{ width: `${msgPct}%` }} />
            )}
            {bufferPct > 0 && (
              <div className={`${SEGMENT_COLORS.buffer} h-full opacity-50`} style={{ width: `${bufferPct}%` }} />
            )}
          </div>
          {/* Threshold marker */}
          {!isCompacting && (
            <div
              className={`absolute top-[-1px] bottom-[-1px] w-[1.5px] rounded-full transition-opacity duration-300 ${
                usedPct >= AUTOCOMPACT_THRESHOLD_PCT
                  ? 'bg-white opacity-90'
                  : 'bg-gray-400 dark:bg-gray-600 opacity-40'
              }`}
              style={{ left: `${AUTOCOMPACT_THRESHOLD_PCT}%` }}
              title="~auto-compact threshold"
            />
          )}
        </div>

        {/* Legend row */}
        <div className="flex items-center gap-3 text-[10px] text-gray-400 dark:text-gray-500">
          <span className="flex items-center gap-1">
            <span className={`inline-block w-2 h-2 rounded-sm ${SEGMENT_COLORS.system}`} />
            System
          </span>
          <span className="flex items-center gap-1">
            <span className={`inline-block w-2 h-2 rounded-sm ${SEGMENT_COLORS.conversation}`} />
            Conversation
          </span>
          <span className="flex items-center gap-1">
            <span className={`inline-block w-2 h-2 rounded-sm ${SEGMENT_COLORS.buffer} opacity-50`} />
            Buffer
          </span>
          <span className="flex items-center gap-1">
            <span className="inline-block w-[1.5px] h-2 rounded-full bg-gray-400 dark:bg-gray-600" />
            ~auto-compact
          </span>
        </div>

        {/* Breakdown rows */}
        <div className="space-y-1 text-[11px]">
          <BreakdownRow label="System (prompt + tools)" tokens={systemEstimate} limit={contextLimit} />
          <BreakdownRow label="Conversation" tokens={messagesEstimate} limit={contextLimit} />
          <BreakdownRow label="Autocompact buffer" tokens={autocompactBuffer} limit={contextLimit} />
          <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
            <div className="flex items-center justify-between font-medium text-gray-900 dark:text-gray-100 text-[11px]">
              <span>Free space</span>
              <span className="tabular-nums font-mono">{formatTokens(freeSpace)}</span>
            </div>
          </div>
        </div>

        {/* Session totals */}
        {tokens && (
          <div className="border-t border-gray-200 dark:border-gray-700 pt-2 space-y-1 text-[11px]">
            <div className="text-gray-400 dark:text-gray-500 text-[10px] uppercase tracking-wide mb-1">Session totals</div>
            <div className="flex justify-between text-gray-500 dark:text-gray-400">
              <span>Total tokens</span>
              <span className="tabular-nums font-mono">{formatTokens(tokens.totalTokens)}</span>
            </div>
            {tokens.cacheReadTokens > 0 && (
              <div className="flex justify-between text-green-600 dark:text-green-400">
                <span>Cache read</span>
                <span className="tabular-nums font-mono">{formatTokens(tokens.cacheReadTokens)} ({cacheEfficiency}%)</span>
              </div>
            )}
            {tokens.cacheCreationTokens > 0 && (
              <div className="flex justify-between text-gray-500 dark:text-gray-400">
                <span>Cache written</span>
                <span className="tabular-nums font-mono">{formatTokens(tokens.cacheCreationTokens)}</span>
              </div>
            )}
            <div className="flex justify-between text-gray-500 dark:text-gray-400">
              <span>Output</span>
              <span className="tabular-nums font-mono">{formatTokens(tokens.outputTokens)}</span>
            </div>
            {turnCount != null && turnCount > 0 && (
              <div className="flex justify-between text-gray-500 dark:text-gray-400">
                <span>Turns</span>
                <span className="tabular-nums font-mono">{turnCount}</span>
              </div>
            )}
          </div>
        )}

        {/* Hint */}
        <div className="text-[10px] text-gray-400 dark:text-gray-500 italic">
          Run <span className="font-mono text-gray-500 dark:text-gray-400 not-italic">/context</span> in session for full breakdown
        </div>
      </div>
    )
  }

  // ---- Compact mode: bar + hover tooltip (unchanged for session cards) ----
  return (
    <div ref={containerRef} className="relative space-y-1" onMouseEnter={handleMouseEnter} onMouseLeave={handleMouseLeave}>
      <div className={`relative h-1.5 rounded-full bg-gray-200 dark:bg-gray-800${isCompacting ? ' motion-safe:animate-pulse' : ''}`}>
        <div className="h-full rounded-full overflow-hidden">
          {usedPct > 0 && (
            <div
              className={`${barColor} h-full transition-all duration-300`}
              style={{ width: `${usedPct}%` }}
            />
          )}
        </div>
        {/* Threshold marker */}
        {!isCompacting && (
          <div
            className={`absolute top-[-1px] bottom-[-1px] w-[1.5px] rounded-full transition-opacity duration-300 ${
              usedPct >= AUTOCOMPACT_THRESHOLD_PCT
                ? 'bg-white opacity-90'
                : 'bg-gray-400 dark:bg-gray-600 opacity-40'
            }`}
            style={{ left: `${AUTOCOMPACT_THRESHOLD_PCT}%` }}
            title="~auto-compact threshold"
          />
        )}
      </div>
      <div className="flex items-center justify-between text-[10px] text-gray-400 dark:text-gray-500">
        <span className="flex items-center gap-1">
          {formatTokens(contextWindowTokens)}/{formatTokens(contextLimit)} tokens
          {isCompacting && (
            <>
              <Minimize2 className="h-3 w-3 text-blue-500 dark:text-blue-400 motion-safe:animate-pulse" />
              <span className="text-blue-500 dark:text-blue-400">compacting...</span>
            </>
          )}
          {!isCompacting && justCompacted && (
            <span className="text-green-500 dark:text-green-400 animate-pulse">
              compacted
            </span>
          )}
        </span>
        {usedPct > 75 && (
          <span className={usedPct > 90 ? 'text-red-500' : 'text-amber-500'} title="Context is filling up. Auto-compaction may occur soon.">
            {usedPct.toFixed(0)}% used
          </span>
        )}
      </div>

      {/* Tooltip — portaled to body to escape overflow clipping */}
      {isOpen && createPortal(
        <div ref={tooltipRef} style={tooltipStyle} className="w-64 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-3 text-xs">
          <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">
            Context Window
          </div>

          {/* Model + usage */}
          <div className="flex items-center justify-between text-gray-500 dark:text-gray-400 mb-2">
            <span>{model ?? 'unknown'}</span>
            <span className="tabular-nums font-mono">{usedPct.toFixed(1)}% used</span>
          </div>

          {/* Estimated context breakdown */}
          <div className="space-y-0.5 text-[11px]">
            <div className="text-gray-400 dark:text-gray-500 text-[10px] uppercase tracking-wide mb-1">Estimated breakdown</div>
            <BreakdownRow label="System (prompt + tools)" tokens={systemEstimate} limit={contextLimit} />
            <BreakdownRow label="Conversation" tokens={messagesEstimate} limit={contextLimit} />
            <BreakdownRow label="Autocompact buffer" tokens={autocompactBuffer} limit={contextLimit} />
            <div className="border-t border-gray-200 dark:border-gray-700 pt-1 mt-1">
              <div className="flex items-center justify-between font-medium text-gray-900 dark:text-gray-100">
                <span>Free space</span>
                <span className="tabular-nums font-mono">{formatTokens(freeSpace)}</span>
              </div>
            </div>
          </div>

          {/* Cumulative session tokens */}
          {tokens && (
            <div className="border-t border-gray-200 dark:border-gray-700 pt-2 mt-2 space-y-0.5 text-[11px]">
              <div className="text-gray-400 dark:text-gray-500 text-[10px] uppercase tracking-wide mb-1">Session totals</div>
              <div className="flex justify-between text-gray-500 dark:text-gray-400">
                <span>Total tokens</span>
                <span className="tabular-nums font-mono">{formatTokens(tokens.totalTokens)}</span>
              </div>
              {tokens.cacheReadTokens > 0 && (
                <div className="flex justify-between text-green-600 dark:text-green-400">
                  <span>Cache read</span>
                  <span className="tabular-nums font-mono">{formatTokens(tokens.cacheReadTokens)}</span>
                </div>
              )}
              {tokens.cacheCreationTokens > 0 && (
                <div className="flex justify-between text-gray-500 dark:text-gray-400">
                  <span>Cache written</span>
                  <span className="tabular-nums font-mono">{formatTokens(tokens.cacheCreationTokens)}</span>
                </div>
              )}
              <div className="flex justify-between text-gray-500 dark:text-gray-400">
                <span>Output</span>
                <span className="tabular-nums font-mono">{formatTokens(tokens.outputTokens)}</span>
              </div>
              {turnCount != null && turnCount > 0 && (
                <div className="flex justify-between text-gray-500 dark:text-gray-400">
                  <span>Turns</span>
                  <span className="tabular-nums font-mono">{turnCount}</span>
                </div>
              )}
            </div>
          )}

          {/* Hint */}
          <div className="border-t border-gray-200 dark:border-gray-700 pt-2 mt-2 text-[10px] text-gray-400 dark:text-gray-500 italic">
            Run <span className="font-mono text-gray-500 dark:text-gray-400 not-italic">/context</span> in session for full breakdown
          </div>
        </div>,
        document.body
      )}
    </div>
  )
}

function BreakdownRow({ label, tokens: count, limit }: { label: string; tokens: number; limit: number }) {
  const pct = (count / limit) * 100
  return (
    <div className="flex items-center justify-between text-gray-500 dark:text-gray-400">
      <span>{label}</span>
      <span className="tabular-nums font-mono">
        {formatTokens(count)} <span className="text-gray-400 dark:text-gray-600">({pct.toFixed(1)}%)</span>
      </span>
    </div>
  )
}
