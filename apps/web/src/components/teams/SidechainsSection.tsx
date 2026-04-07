import { CheckCircle2, ChevronRight } from 'lucide-react'
import { useCallback, useRef, useState } from 'react'
import type { TeamMember } from '../../types/generated'
import type { TeamMemberSidechain } from '@claude-view/shared/types/generated/TeamMemberSidechain'
import { formatCostUsd, formatTokenCount } from '../../lib/format-utils'

/** Format seconds into a compact human-readable duration (e.g., "21m", "3m 12s", "34s"). */
function formatCompactDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return s > 0 ? `${m}m ${s}s` : `${m}m`
}

/** Shorten model ID for display (e.g., "claude-opus-4-6" → "opus"). */
function shortModel(model: string): string {
  if (!model) return ''
  const match = model.match(/claude-(\w+)-/)
  return match ? match[1] : model
}

/** Format ISO timestamp to local time (e.g., "10:08 PM"). */
function formatTime(iso: string | null): string {
  if (!iso) return ''
  return new Date(iso).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
}

/** Derive the time range string for a group of sidechains (earliest start → latest end). */
function memberTimeRange(chains: TeamMemberSidechain[]): string {
  const start = chains[0]?.startedAt
  let latestEnd = ''
  for (const c of chains) {
    if (c.endedAt && c.endedAt > latestEnd) latestEnd = c.endedAt
  }
  if (!start || !latestEnd) return ''
  return `${formatTime(start)} – ${formatTime(latestEnd)}`
}

/** Sum costUsd across a group of sidechains. Returns null if none have cost data. */
function memberTotalCost(chains: TeamMemberSidechain[]): number | null {
  let total = 0
  let hasCost = false
  for (const c of chains) {
    if (c.costUsd != null) {
      total += c.costUsd
      hasCost = true
    }
  }
  return hasCost ? total : null
}

const DOT_BG: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-amber-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

const DEFAULT_WIDTH = 224
const MIN_WIDTH = 160
const MAX_WIDTH = 400

export function SidechainsSection({
  byMember,
  members,
  onSelect,
}: {
  byMember: Map<string, TeamMemberSidechain[]>
  members: TeamMember[] | undefined
  onSelect: (target: { hexId: string; memberName: string }) => void
}) {
  const [width, setWidth] = useState(DEFAULT_WIDTH)
  const widthRef = useRef(DEFAULT_WIDTH)
  const [isResizing, setIsResizing] = useState(false)

  const handleResizeStart = useCallback((e: React.PointerEvent<HTMLDivElement>) => {
    e.preventDefault()
    setIsResizing(true)
    const startX = e.clientX
    const startW = widthRef.current

    const onMove = (ev: PointerEvent) => {
      const delta = startX - ev.clientX
      const newWidth = Math.round(Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startW + delta)))
      widthRef.current = newWidth
      setWidth(newWidth)
    }
    const onUp = () => {
      setIsResizing(false)
      document.removeEventListener('pointermove', onMove)
      document.removeEventListener('pointerup', onUp)
    }
    document.addEventListener('pointermove', onMove)
    document.addEventListener('pointerup', onUp)
  }, [])

  return (
    <div
      className={`relative flex-shrink-0 overflow-y-auto ${isResizing ? 'select-none' : ''}`}
      style={{ width }}
    >
      {/* Resize handle (left edge) */}
      <div
        onPointerDown={handleResizeStart}
        className="absolute top-0 left-0 w-1.5 h-full cursor-col-resize z-10 group"
      >
        <div className="w-px h-full mx-auto bg-gray-200 dark:bg-gray-800 group-hover:bg-amber-500/40 group-active:bg-amber-500/60 transition-colors" />
      </div>

      <h4 className="px-3 pt-2 pb-1 text-[10px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider">
        Member Sessions
      </h4>

      {[...byMember.entries()].map(([member, chains]) => {
        const model = shortModel(chains[0]?.model ?? '')
        const memberInfo = members?.find((m) => m.name === member)
        const dotColor = DOT_BG[memberInfo?.color ?? ''] ?? 'bg-gray-400'
        const timeRange = memberTimeRange(chains)
        const totalCost = memberTotalCost(chains)
        return (
          <div
            key={member}
            className="px-3 py-1.5 border-b border-gray-100 dark:border-gray-800/50 last:border-b-0"
          >
            {/* Member header: colored dot + name + rolled-up cost */}
            <div className="flex items-center gap-1.5 mb-0.5">
              <span className={`w-2 h-2 rounded-full shrink-0 ${dotColor}`} />
              <p className="text-xs font-medium text-gray-700 dark:text-gray-300">{member}</p>
              {totalCost != null && totalCost > 0 && (
                <span className="ml-auto text-[10px] font-mono tabular-nums text-gray-500 dark:text-gray-400">
                  {formatCostUsd(totalCost)}
                </span>
              )}
            </div>
            {/* Meta line: model badge + session count */}
            <div className="flex items-center gap-1.5 mb-1.5 pl-3.5">
              {model && (
                <span className="inline-flex items-center px-1 py-px rounded text-[10px] font-medium bg-sky-50 text-sky-600 dark:bg-sky-900/30 dark:text-sky-400">
                  {model}
                </span>
              )}
              <span className="text-[10px] text-gray-400 dark:text-gray-500">
                {chains.length} {chains.length === 1 ? 'session' : 'sessions'}
              </span>
            </div>
            {timeRange && (
              <p className="text-[10px] text-gray-400 dark:text-gray-500 pl-3.5 -mt-1 mb-1.5">
                {timeRange}
              </p>
            )}

            {/* Sidechain rows */}
            {chains.map((sc) => {
              const isShort = sc.durationSeconds < 60
              const hasCost = sc.costUsd != null && sc.costUsd > 0
              const hasTokens =
                sc.tokens != null &&
                (sc.tokens.inputTokens > 0 ||
                  sc.tokens.outputTokens > 0 ||
                  sc.tokens.cacheReadTokens > 0)
              return (
                <button
                  key={sc.hexId}
                  type="button"
                  onClick={() => onSelect({ hexId: sc.hexId, memberName: sc.memberName })}
                  className="group w-full flex flex-col px-1.5 py-1 rounded text-left hover:bg-gray-50 dark:hover:bg-gray-800/60 transition-colors"
                >
                  <div className="flex items-center gap-1.5 w-full">
                    <CheckCircle2
                      className={`w-3 h-3 flex-shrink-0 ${
                        isShort
                          ? 'text-amber-400 dark:text-amber-500'
                          : 'text-green-500 dark:text-green-400'
                      }`}
                    />
                    <span className="text-xs tabular-nums text-gray-600 dark:text-gray-400 min-w-[2.5rem]">
                      {formatCompactDuration(sc.durationSeconds)}
                    </span>
                    {hasCost ? (
                      <span className="text-[10px] font-mono tabular-nums text-gray-500 dark:text-gray-400">
                        {formatCostUsd(sc.costUsd!)}
                      </span>
                    ) : (
                      <span className="text-[10px] text-gray-400 dark:text-gray-500">
                        {sc.lineCount} lines
                      </span>
                    )}
                    <ChevronRight className="w-3 h-3 ml-auto flex-shrink-0 text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100 transition-opacity" />
                  </div>
                  {hasTokens && (
                    <div className="ml-[1.125rem] text-[10px] text-gray-400 dark:text-gray-500 font-mono tabular-nums">
                      {[
                        sc.tokens!.inputTokens > 0
                          ? `${formatTokenCount(sc.tokens!.inputTokens)} in`
                          : null,
                        sc.tokens!.outputTokens > 0
                          ? `${formatTokenCount(sc.tokens!.outputTokens)} out`
                          : null,
                        sc.tokens!.cacheReadTokens > 0
                          ? `${formatTokenCount(sc.tokens!.cacheReadTokens)} cache`
                          : null,
                      ]
                        .filter(Boolean)
                        .join(' · ')}
                    </div>
                  )}
                </button>
              )
            })}
          </div>
        )
      })}
    </div>
  )
}
