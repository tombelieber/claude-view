import { TurnDurationCard } from '@claude-view/shared/components/TurnDurationCard'
import type { TurnBoundaryBlock } from '@claude-view/shared/types/blocks'
import { JsonTree } from '../../../live/JsonTree'
import { useJsonMode } from './json-mode-context'

interface TurnBoundaryProps {
  block: TurnBoundaryBlock
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}

function formatCost(usd: number): string {
  if (usd <= 0) return '$0'
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  return `$${usd.toFixed(3)}`
}

function formatTokens(usage: Record<string, number>): string {
  const input = usage.input_tokens ?? usage.inputTokens ?? 0
  const output = usage.output_tokens ?? usage.outputTokens ?? 0
  if (input === 0 && output === 0) return ''
  return `${input.toLocaleString()} in / ${output.toLocaleString()} out`
}

export function DevTurnBoundary({ block }: TurnBoundaryProps) {
  const globalJsonMode = useJsonMode()
  const models = Object.keys(block.modelUsage)
  const tokens = formatTokens(block.usage)

  if (globalJsonMode) {
    return (
      <div className="overflow-hidden rounded-lg border border-gray-200/30 dark:border-gray-700/30">
        <div className="px-3 py-2">
          <JsonTree data={block} defaultExpandDepth={3} verboseMode />
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-1">
      <TurnDurationCard durationMs={block.durationMs} />
      <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-1.5 bg-gray-50 dark:bg-gray-800/40 border-b border-gray-200/50 dark:border-gray-700/50">
          <span className="text-[10px] font-medium text-gray-500 dark:text-gray-400">
            Turn {block.numTurns}
          </span>
          {!block.success && (
            <span className="text-[10px] font-medium text-red-500 dark:text-red-400 px-1.5 py-0.5 rounded bg-red-50 dark:bg-red-900/20">
              {block.error?.subtype ?? 'Error'}
            </span>
          )}
        </div>

        {/* Stats grid */}
        <div className="grid grid-cols-2 gap-x-4 gap-y-1 px-3 py-2 text-[11px]">
          <div className="flex justify-between">
            <span className="text-gray-500 dark:text-gray-400">Cost</span>
            <span className="font-mono text-gray-700 dark:text-gray-300">
              {formatCost(block.totalCostUsd)}
            </span>
          </div>
          <div className="flex justify-between">
            <span className="text-gray-500 dark:text-gray-400">Duration</span>
            <span className="font-mono text-gray-700 dark:text-gray-300">
              {formatDuration(block.durationMs)}
            </span>
          </div>
          {tokens && (
            <div className="flex justify-between col-span-2">
              <span className="text-gray-500 dark:text-gray-400">Tokens</span>
              <span className="font-mono text-gray-700 dark:text-gray-300">{tokens}</span>
            </div>
          )}
          {models.length > 0 && (
            <div className="flex justify-between col-span-2">
              <span className="text-gray-500 dark:text-gray-400">Model</span>
              <span className="font-mono text-gray-700 dark:text-gray-300 truncate max-w-[200px]">
                {models.join(', ')}
              </span>
            </div>
          )}
          {block.stopReason && (
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Stop</span>
              <span className="font-mono text-gray-700 dark:text-gray-300">{block.stopReason}</span>
            </div>
          )}
          {block.fastModeState && (
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">Fast</span>
              <span className="font-mono text-gray-700 dark:text-gray-300">
                {block.fastModeState}
              </span>
            </div>
          )}
          {block.durationApiMs != null && block.durationApiMs > 0 && (
            <div className="flex justify-between">
              <span className="text-gray-500 dark:text-gray-400">API</span>
              <span className="font-mono text-gray-700 dark:text-gray-300">
                {formatDuration(block.durationApiMs)}
              </span>
            </div>
          )}
        </div>

        {/* Permission denials */}
        {block.permissionDenials.length > 0 && (
          <div className="px-3 py-1.5 border-t border-gray-200/30 dark:border-gray-700/30">
            <div className="text-[10px] font-medium text-red-600 dark:text-red-400 mb-1">
              Permission Denials
            </div>
            {block.permissionDenials.map((d) => (
              <div
                key={d.toolUseId}
                className="text-[10px] font-mono text-gray-600 dark:text-gray-400"
              >
                {d.toolName}
              </div>
            ))}
          </div>
        )}

        {/* Error messages */}
        {block.error && (
          <div className="px-3 py-1.5 border-t border-red-200/30 dark:border-red-800/30 bg-red-50/30 dark:bg-red-900/10">
            {block.error.messages.map((msg) => (
              <p key={msg} className="text-[10px] text-red-600 dark:text-red-400">
                {msg}
              </p>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
