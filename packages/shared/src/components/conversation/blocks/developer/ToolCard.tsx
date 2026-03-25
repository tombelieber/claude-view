import { ChevronDown, ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { useDeveloperTools } from '../../../../contexts/DeveloperToolsContext'
import type { ToolExecution } from '../../../../types/blocks'
import { cn } from '../../../../utils/cn'
import { shortenToolName, toolChipColor } from '../../../../utils/content-detection'
import { CopyButton } from '../shared/CopyButton'
import { ContentRenderer } from './ContentRenderer'
import { DurationBadge } from './DurationBadge'
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
        'overflow-hidden rounded-lg border transition-colors duration-200 shadow-[0_1px_2px_rgba(0,0,0,0.04)] dark:shadow-[0_1px_3px_rgba(0,0,0,0.2)]',
        execution.status === 'error'
          ? 'border-red-300/25 dark:border-red-800/40 bg-red-500/5 dark:bg-red-950/20'
          : 'border-gray-200/30 dark:border-gray-700/30',
      )}
    >
      {/* ── Header ──────────────────────────────────────────────── */}
      {/* biome-ignore lint/a11y/useSemanticElements: div-as-button for compound clickable row with nested interactive children */}
      <div
        role="button"
        tabIndex={0}
        onClick={() => hasContent && setExpanded(!expanded)}
        onKeyDown={(e) => {
          if ((e.key === 'Enter' || e.key === ' ') && hasContent) {
            e.preventDefault()
            setExpanded(!expanded)
          }
        }}
        className={cn(
          'flex w-full items-center gap-2 px-3 py-2',
          hasContent && 'cursor-pointer hover:bg-gray-500/5 transition-colors duration-200',
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
          <span className="text-[9px] font-mono text-gray-500 dark:text-gray-400 flex-shrink-0">
            {server}
          </span>
        )}

        {execution.category && (
          <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-gray-500/10 dark:bg-gray-500/20 text-gray-600 dark:text-gray-400">
            {execution.category}
          </span>
        )}

        {/* Smart label */}
        {smartLabel && (
          <span
            className="text-xs text-gray-500 dark:text-gray-400 font-mono truncate"
            title={smartLabel}
          >
            {smartLabel}
          </span>
        )}

        {/* Spacer */}
        <span className="flex-1" />

        {/* Running elapsed */}
        {execution.status === 'running' && execution.progress && (
          <span className="text-[10px] font-mono text-blue-600 dark:text-blue-400 tabular-nums flex-shrink-0">
            {execution.progress.elapsedSeconds.toFixed(1)}s
          </span>
        )}

        {execution.duration != null && <DurationBadge ms={execution.duration} />}

        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            setLocalOverride((v) => !(v ?? globalJsonMode))
          }}
          className={cn(
            'text-[10px] font-mono px-1.5 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
            'min-w-[28px] min-h-[28px] inline-flex items-center justify-center',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50',
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
              <ChevronDown className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400" />
            ) : (
              <ChevronRight className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400" />
            )}
          </span>
        )}
      </div>

      {/* ── Body ────────────────────────────────────────────────── */}
      <div
        className="grid transition-[grid-template-rows] duration-200 ease-out"
        style={{ gridTemplateRows: expanded ? '1fr' : '0fr' }}
      >
        <div className="overflow-hidden">
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
                            ? 'text-red-600 dark:text-red-400'
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
        </div>
      </div>
    </div>
  )
}
