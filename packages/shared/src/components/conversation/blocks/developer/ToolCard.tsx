import type { ToolExecution } from '../../../../types/blocks'
import { Check, ChevronDown, ChevronRight, Copy } from 'lucide-react'
import { useCallback, useState } from 'react'
import { useDeveloperTools } from '../../../../contexts/DeveloperToolsContext'
import { shortenToolName, toolChipColor } from '../../../../utils/content-detection'
import { cn } from '../../../../utils/cn'
import { ContentRenderer } from './ContentRenderer'
import { SimpleJsonView } from './SimpleJsonView'
import { useJsonMode } from './json-mode-context'

// ── Status dot (ActionLog pattern) ──────────────────────────────────────────

function StatusDot({ status }: { status: ToolExecution['status'] }) {
  const color =
    status === 'complete'
      ? 'bg-green-500'
      : status === 'error'
        ? 'bg-red-500'
        : 'bg-amber-400 animate-pulse'
  return (
    <span
      data-testid={`status-${status}`}
      className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', color)}
    />
  )
}

// ── Color-coded duration (ActionLog pattern) ────────────────────────────────

function Duration({ ms }: { ms: number | undefined }) {
  if (ms == null) return null
  const secs = ms / 1000
  const text = secs >= 60 ? `${(secs / 60).toFixed(1)}m` : `${secs.toFixed(1)}s`
  const color =
    secs > 30
      ? 'text-red-400 bg-red-500/10'
      : secs > 5
        ? 'text-amber-400 bg-amber-500/10'
        : 'text-gray-400 bg-gray-500/10'
  return (
    <span className={cn('text-[9px] font-mono tabular-nums px-1.5 py-0.5 rounded', color)}>
      {text}
    </span>
  )
}

// ── Smart label from toolInput ──────────────────────────────────────────────

function extractLabel(toolName: string, toolInput: Record<string, unknown>): string | null {
  const fp = toolInput.file_path as string | undefined
  if (fp) {
    const parts = fp.split('/')
    return parts.length > 2 ? `.../${parts.slice(-2).join('/')}` : fp
  }
  if (toolName === 'Bash') {
    const cmd = (toolInput.command as string) || ''
    const first = cmd.split('\n')[0]
    return first.length > 60 ? `${first.slice(0, 57)}...` : first
  }
  if (toolName === 'Grep') return (toolInput.pattern as string) || null
  if (toolName === 'Glob') return (toolInput.pattern as string) || null
  if (toolName === 'Skill') return (toolInput.skill as string) || null
  if (toolName === 'Task') {
    const desc = (toolInput.description as string) || (toolInput.prompt as string) || ''
    return desc.length > 50 ? `${desc.slice(0, 47)}...` : desc || null
  }
  return null
}

// ── Copy button ─────────────────────────────────────────────────────────────

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    })
  }, [text])
  return (
    <button
      onClick={(e) => {
        e.stopPropagation()
        handleCopy()
      }}
      className="text-gray-500 hover:text-gray-300 transition-colors duration-200 p-0.5 cursor-pointer"
      title="Copy to clipboard"
    >
      {copied ? <Check className="w-3 h-3 text-green-400" /> : <Copy className="w-3 h-3" />}
    </button>
  )
}

// ── Main ToolCard ───────────────────────────────────────────────────────────

interface ToolCardProps {
  execution: ToolExecution
}

export function ToolCard({ execution }: ToolCardProps) {
  const { JsonTree, getToolRenderer } = useDeveloperTools()
  const globalJsonMode = useJsonMode()
  const [expanded, setExpanded] = useState(true)
  const [localOverride, setLocalOverride] = useState<boolean | null>(null)
  const jsonMode = localOverride ?? globalJsonMode
  const hasContent = !!execution.result || Object.keys(execution.toolInput ?? {}).length > 0

  const { short: label, server } = shortenToolName(execution.toolName)
  const chipColor = toolChipColor(execution.toolName)
  const smartLabel = extractLabel(execution.toolName, execution.toolInput ?? {})
  const RichRenderer = getToolRenderer?.(execution.toolName) ?? null

  return (
    <div
      className={cn(
        'overflow-hidden rounded-lg border transition-colors duration-200',
        execution.status === 'error'
          ? 'border-red-500/30 bg-red-500/5'
          : 'border-gray-200/30 dark:border-gray-700/30',
      )}
    >
      {/* ── Header ──────────────────────────────────────────────── */}
      <div
        role="button"
        tabIndex={0}
        onClick={() => hasContent && setExpanded(!expanded)}
        onKeyDown={(e) => e.key === 'Enter' && hasContent && setExpanded(!expanded)}
        className={cn(
          'flex w-full items-center gap-2 px-3 py-2',
          hasContent && 'cursor-pointer hover:bg-gray-50/5 transition-colors duration-200',
        )}
      >
        <StatusDot status={execution.status} />

        {/* Tool name chip */}
        <span
          className={cn(
            'inline-flex items-center px-2 py-0.5 rounded text-[10px] font-mono font-semibold flex-shrink-0',
            chipColor,
          )}
        >
          {label}
        </span>

        {server && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600 flex-shrink-0">
            {server}
          </span>
        )}

        {execution.category && (
          <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-gray-500/10 text-gray-400">
            {execution.category}
          </span>
        )}

        {/* Smart label */}
        {smartLabel && (
          <span
            className="text-xs text-gray-400 dark:text-gray-500 font-mono truncate"
            title={smartLabel}
          >
            {smartLabel}
          </span>
        )}

        {/* Spacer */}
        <span className="flex-1" />

        {/* Running elapsed */}
        {execution.status === 'running' && execution.progress && (
          <span className="text-[10px] font-mono text-blue-400 tabular-nums animate-pulse flex-shrink-0">
            {execution.progress.elapsedSeconds.toFixed(1)}s
          </span>
        )}

        <Duration ms={execution.duration} />

        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            setLocalOverride((v) => ((v ?? globalJsonMode) ? false : true))
          }}
          className={cn(
            'text-[10px] font-mono px-1.5 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
            jsonMode
              ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
              : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
          )}
          title={jsonMode ? 'Switch to rich view' : 'Switch to JSON view'}
        >
          {'{ }'}
        </button>

        {hasContent && (
          <span className="flex-shrink-0">
            {expanded ? (
              <ChevronDown className="w-3.5 h-3.5 text-gray-500" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-gray-500" />
            )}
          </span>
        )}
      </div>

      {/* ── Body ────────────────────────────────────────────────── */}
      {expanded && (
        <div className="border-t border-gray-200/20 dark:border-gray-700/20 px-3 py-2.5 space-y-2.5">
          {jsonMode ? (
            JsonTree ? (
              <JsonTree data={execution} defaultExpandDepth={3} verboseMode />
            ) : (
              <SimpleJsonView data={execution} />
            )
          ) : (
            <>
              {/* Input: rich renderer if available, else raw JSON */}
              {Object.keys(execution.toolInput ?? {}).length > 0 && (
                <div>
                  {RichRenderer ? (
                    <RichRenderer
                      inputData={execution.toolInput as Record<string, unknown>}
                      name={execution.toolName}
                      blockIdPrefix={`dev-${execution.toolUseId}-`}
                    />
                  ) : (
                    <>
                      <div className="mb-1 text-[9px] font-medium uppercase tracking-wider text-gray-500 dark:text-gray-500">
                        Input
                      </div>
                      <ContentRenderer content={JSON.stringify(execution.toolInput, null, 2)} />
                    </>
                  )}
                </div>
              )}

              {/* Result */}
              {execution.result && (
                <div>
                  <div className="flex items-center gap-1.5 mb-1">
                    <span
                      className={cn(
                        'text-[9px] font-medium uppercase tracking-wider',
                        execution.result.isError
                          ? 'text-red-400'
                          : 'text-gray-500 dark:text-gray-500',
                      )}
                    >
                      {execution.result.isError ? 'Error' : 'Output'}
                    </span>
                    <CopyButton text={execution.result.output} />
                  </div>
                  <ContentRenderer content={execution.result.output} />
                </div>
              )}

              {/* Summary */}
              {execution.summary && (
                <p className="text-[10px] italic text-gray-500 dark:text-gray-400">
                  {execution.summary}
                </p>
              )}
            </>
          )}
        </div>
      )}
    </div>
  )
}
