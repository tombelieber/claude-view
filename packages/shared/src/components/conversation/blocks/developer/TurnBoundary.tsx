import type { TurnBoundaryBlock } from '../../../../types/blocks'
import { TurnDurationCard } from '../../../TurnDurationCard'
import { EventCard } from './EventCard'

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
  const models = Object.keys(block.modelUsage)
  const tokens = formatTokens(block.usage)

  return (
    <EventCard
      dot={block.success ? 'green' : 'red'}
      chip="Turn"
      label={`Turn ${block.numTurns} — ${formatCost(block.totalCostUsd)} / ${formatDuration(block.durationMs)}`}
      error={!block.success}
      rawData={block}
    >
      <div className="space-y-2">
        <TurnDurationCard durationMs={block.durationMs} />

        {/* Compact stat bar */}
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[10px] font-mono text-gray-600 dark:text-gray-300">
          <span>
            <span className="text-gray-500 dark:text-gray-400">Cost</span>{' '}
            {formatCost(block.totalCostUsd)}
          </span>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span>
            <span className="text-gray-500 dark:text-gray-400">Duration</span>{' '}
            {formatDuration(block.durationMs)}
          </span>
          {tokens && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span>
                <span className="text-gray-500 dark:text-gray-400">Tokens</span> {tokens}
              </span>
            </>
          )}
          {models.length > 0 && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span className="truncate max-w-[200px]">
                <span className="text-gray-500 dark:text-gray-400">Model</span> {models.join(', ')}
              </span>
            </>
          )}
          {block.stopReason && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span>
                <span className="text-gray-500 dark:text-gray-400">Stop</span> {block.stopReason}
              </span>
            </>
          )}
          {block.fastModeState && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span>
                <span className="text-gray-500 dark:text-gray-400">Fast</span> {block.fastModeState}
              </span>
            </>
          )}
          {block.durationApiMs != null && block.durationApiMs > 0 && (
            <>
              <span className="text-gray-300 dark:text-gray-600">|</span>
              <span>
                <span className="text-gray-500 dark:text-gray-400">API</span>{' '}
                {formatDuration(block.durationApiMs)}
              </span>
            </>
          )}
        </div>

        {/* Permission denials */}
        {block.permissionDenials.length > 0 && (
          <div className="pt-1.5 border-t border-gray-200/30 dark:border-gray-700/30">
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
          <div className="pt-1.5 border-t border-red-200/30 dark:border-red-800/30">
            {block.error.messages.map((msg) => (
              <p key={msg} className="text-[10px] text-red-600 dark:text-red-400">
                {msg}
              </p>
            ))}
          </div>
        )}
      </div>
    </EventCard>
  )
}
