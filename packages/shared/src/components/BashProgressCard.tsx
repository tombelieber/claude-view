import { Terminal } from 'lucide-react'
import { useState } from 'react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * BashProgressCard — purpose-built for BashProgress schema.
 *
 * Schema fields: output, fullOutput, elapsedTimeSeconds, totalLines, totalBytes, taskId
 * Every field is rendered. No phantom props.
 */

interface BashProgressCardProps {
  /** Recent output lines (tail of the running command) */
  output: string
  /** Complete output from start to current point */
  fullOutput: string
  /** How long the command has been running */
  elapsedTimeSeconds: number
  /** Total number of output lines so far */
  totalLines: number
  /** Total bytes of output (bigint from Rust u64, arrives as number via JSON) */
  totalBytes: number | bigint
  /** Background task ID, if this bash command is associated with one */
  taskId?: string | null
  /** UI-only: stable key for code block rendering */
  blockId?: string
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

export function BashProgressCard({
  output,
  fullOutput,
  elapsedTimeSeconds,
  totalLines,
  totalBytes,
  taskId,
  blockId,
}: BashProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()
  const [showFull, setShowFull] = useState(false)

  const bytesNum = Number(totalBytes)
  const displayOutput = showFull ? fullOutput : output
  const canExpand = fullOutput.length > output.length

  return (
    <div data-testid="bash-progress-card" className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1">
      {/* Stats bar — elapsed, lines, bytes, taskId */}
      <div className="flex items-center gap-1.5 mb-0.5 flex-wrap">
        <Terminal className="w-3 h-3 text-gray-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400">
          {elapsedTimeSeconds.toFixed(1)}s
        </span>
        <span className="text-xs text-gray-600 dark:text-gray-600" aria-hidden="true">
          &middot;
        </span>
        <span className="text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400">
          {totalLines.toLocaleString()} {totalLines === 1 ? 'line' : 'lines'}
        </span>
        <span className="text-xs text-gray-600 dark:text-gray-600" aria-hidden="true">
          &middot;
        </span>
        <span className="text-xs font-mono tabular-nums text-gray-500 dark:text-gray-400">
          {formatBytes(bytesNum)}
        </span>
        {taskId && (
          <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-gray-500/10 dark:bg-gray-500/20 text-gray-500 dark:text-gray-400 flex-shrink-0">
            task:{taskId}
          </span>
        )}
      </div>

      {/* Output */}
      {displayOutput ? (
        <CompactCodeBlock
          code={displayOutput}
          language="bash"
          blockId={blockId ? `${blockId}-out` : undefined}
        />
      ) : (
        <div className="px-2 py-1.5 text-xs text-gray-500 dark:text-gray-400 font-mono">
          No output
        </div>
      )}

      {/* Expand toggle: recent ↔ full output */}
      {canExpand && (
        <button
          onClick={() => setShowFull(!showFull)}
          className="text-xs font-mono text-gray-400 hover:text-gray-300 dark:hover:text-gray-300 mt-0.5 cursor-pointer"
        >
          {showFull
            ? '\u25BE recent output'
            : `\u25B8 full output (${totalLines.toLocaleString()} lines)`}
        </button>
      )}
    </div>
  )
}
