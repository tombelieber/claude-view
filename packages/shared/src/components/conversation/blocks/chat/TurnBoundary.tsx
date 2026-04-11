import type { TurnBoundaryBlock } from '../../../../types/blocks'
import { AlertTriangle, ChevronDown, ChevronRight, ShieldOff } from 'lucide-react'
import { useState } from 'react'
import { CollapsibleJson } from '../shared/CollapsibleJson'
import { StatusBadge } from '../shared/StatusBadge'

interface TurnBoundaryProps {
  block: TurnBoundaryBlock
}

function formatCost(usd: number): string {
  if (usd <= 0) return ''
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(3)}`
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return `${n}`
}

/** Extract short model label from model key (e.g. "claude-sonnet-4-5-20250514" → "sonnet-4.5") */
function shortModelName(key: string): string {
  const m = key.match(/claude-(\w+)-(\d+)-(\d+)/)
  if (m) return `${m[1]}-${m[2]}.${m[3]}`
  return key.replace(/^claude-/, '').replace(/-\d{8}$/, '')
}

/** Build the compact info segments for the divider line. */
function buildInfoSegments(block: TurnBoundaryBlock): string[] {
  const segments: string[] = []

  const modelKeys = Object.keys(block.modelUsage ?? {})
  if (modelKeys.length > 0) {
    segments.push(shortModelName(modelKeys[0]))
  }

  const usage = Object.values(block.modelUsage ?? {})
  if (usage.length > 0) {
    const totalIn = usage.reduce((sum, u) => sum + u.inputTokens, 0)
    const totalOut = usage.reduce((sum, u) => sum + u.outputTokens, 0)
    if (totalIn > 0 || totalOut > 0) {
      segments.push(`${formatTokens(totalIn)}→${formatTokens(totalOut)}`)
    }
  }

  const costLabel = formatCost(block.totalCostUsd)
  if (costLabel) segments.push(costLabel)

  if (block.durationMs > 0) segments.push(formatDuration(block.durationMs))

  return segments
}

function fastModeColor(state: string): 'green' | 'amber' | 'gray' {
  if (state === 'on') return 'green'
  if (state === 'cooldown') return 'amber'
  return 'gray'
}

export function ChatTurnBoundary({ block }: TurnBoundaryProps) {
  const infoSegments = buildInfoSegments(block)
  const hasHookErrors = block.hookErrors && block.hookErrors.length > 0
  const [detailsOpen, setDetailsOpen] = useState(false)

  const hasDetails =
    !!block.stopReason ||
    !!block.fastModeState ||
    (block.durationApiMs != null && block.durationApiMs > 0) ||
    block.permissionDenials.length > 0 ||
    !!block.result ||
    block.structuredOutput != null ||
    (block.hookInfos?.length ?? 0) > 0 ||
    !!block.error

  return (
    <div className="space-y-0">
      {/* Main divider line with model · tokens · cost · duration */}
      <div className="flex items-center gap-2 py-1">
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
        {infoSegments.length > 0 && (
          <span className="text-xs font-mono text-gray-400 dark:text-gray-500 tabular-nums">
            {infoSegments.join(' · ')}
          </span>
        )}
        {!block.success && !hasHookErrors && !block.preventedContinuation && (
          <span className="text-xs font-medium text-red-500 dark:text-red-400 px-1.5 py-0.5 rounded bg-red-50 dark:bg-red-900/20">
            {block.error?.subtype === 'error_max_turns'
              ? `Max turns (${block.numTurns})`
              : block.error?.subtype === 'error_max_budget_usd'
                ? 'Budget exceeded'
                : 'Error'}
          </span>
        )}
        <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      </div>

      {/* Collapsible details section */}
      {hasDetails && (
        <div className="px-4 pb-1">
          <button
            type="button"
            onClick={() => setDetailsOpen(!detailsOpen)}
            className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-400 transition-colors cursor-pointer"
          >
            {detailsOpen ? (
              <ChevronDown className="w-3 h-3" />
            ) : (
              <ChevronRight className="w-3 h-3" />
            )}
            Details
          </button>
          {detailsOpen && (
            <div className="mt-1 space-y-1.5 text-xs">
              {/* stopReason */}
              {block.stopReason && (
                <div className="flex items-center gap-2">
                  <span className="text-gray-500 dark:text-gray-400">Stop:</span>
                  <StatusBadge label={block.stopReason} color="gray" />
                </div>
              )}

              {/* fastModeState */}
              {block.fastModeState && (
                <div className="flex items-center gap-2">
                  <span className="text-gray-500 dark:text-gray-400">Fast mode:</span>
                  <StatusBadge
                    label={block.fastModeState}
                    color={fastModeColor(block.fastModeState)}
                  />
                </div>
              )}

              {/* durationApiMs */}
              {block.durationApiMs != null && block.durationApiMs > 0 && (
                <div className="flex items-center gap-2">
                  <span className="text-gray-500 dark:text-gray-400">API duration:</span>
                  <span className="font-mono text-gray-600 dark:text-gray-300">
                    {formatDuration(block.durationApiMs)}
                  </span>
                </div>
              )}

              {/* error subtype + messages */}
              {block.error && (
                <div className="space-y-1">
                  <div className="flex items-center gap-2">
                    <span className="text-gray-500 dark:text-gray-400">Error:</span>
                    <StatusBadge label={block.error.subtype} color="red" />
                  </div>
                  {block.error.messages.length > 0 && (
                    <ul className="space-y-0.5 pl-2">
                      {block.error.messages.map((msg) => (
                        <li key={msg} className="text-red-600 dark:text-red-400">
                          {msg}
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}

              {/* result */}
              {block.result && (
                <div className="flex items-start gap-2">
                  <span className="text-gray-500 dark:text-gray-400 shrink-0">Result:</span>
                  <span className="text-gray-700 dark:text-gray-300 break-all">{block.result}</span>
                </div>
              )}

              {/* structuredOutput */}
              {block.structuredOutput != null && (
                <CollapsibleJson data={block.structuredOutput} label="structuredOutput" />
              )}

              {/* hookInfos */}
              {block.hookInfos && block.hookInfos.length > 0 && (
                <CollapsibleJson
                  data={block.hookInfos}
                  label={`hookInfos (${block.hookInfos.length})`}
                />
              )}

              {/* permissionDenials */}
              {block.permissionDenials.length > 0 && (
                <div className="space-y-1.5">
                  <span className="text-gray-500 dark:text-gray-400">
                    Permission denials ({block.permissionDenials.length}):
                  </span>
                  {block.permissionDenials.map((d) => (
                    <div
                      key={d.toolUseId}
                      className="pl-2 space-y-1 border-l-2 border-red-200 dark:border-red-800/40"
                    >
                      <div className="flex items-center gap-2">
                        <StatusBadge label={d.toolName} color="red" />
                        <span className="font-mono text-gray-500 dark:text-gray-400 truncate">
                          {d.toolUseId}
                        </span>
                      </div>
                      {Object.keys(d.toolInput ?? {}).length > 0 && (
                        <CollapsibleJson data={d.toolInput} label="toolInput" />
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Hook error details */}
      {hasHookErrors && (
        <div className="flex items-start gap-1.5 px-4 py-1.5 mx-auto max-w-[90%] rounded bg-amber-50 dark:bg-gray-800 border border-amber-200 dark:border-amber-600/30">
          <AlertTriangle className="w-3 h-3 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
          <div className="space-y-0.5">
            {block.hookErrors!.map((err) => (
              <p key={err} className="text-xs text-amber-700 dark:text-amber-300/90">
                {err}
              </p>
            ))}
          </div>
        </div>
      )}

      {/* Prevented continuation */}
      {block.preventedContinuation && (
        <div className="flex items-center gap-1.5 px-4 py-1.5 mx-auto max-w-[90%] rounded bg-orange-50 dark:bg-gray-800 border border-orange-200 dark:border-orange-500/30">
          <ShieldOff className="w-3 h-3 text-orange-600 dark:text-orange-400 flex-shrink-0" />
          <span className="text-xs text-orange-700 dark:text-orange-300/90">
            Hook blocked continuation
          </span>
          {block.hookCount != null && (
            <span className="text-xs font-mono text-orange-600 dark:text-orange-400 ml-auto tabular-nums">
              {block.hookCount} hook{block.hookCount !== 1 ? 's' : ''} ran
            </span>
          )}
        </div>
      )}
    </div>
  )
}
