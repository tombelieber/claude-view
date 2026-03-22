import type { ToolExecution } from '@claude-view/shared/types/blocks'
import { Check, Loader2, Wrench, X } from 'lucide-react'

interface ToolChipProps {
  execution: ToolExecution
}

function getToolPreview(execution: ToolExecution): string {
  const { toolName, toolInput } = execution
  switch (toolName) {
    case 'Read':
      return (
        String(toolInput.file_path ?? toolInput.filePath ?? '')
          .split('/')
          .pop() ?? ''
      )
    case 'Write':
    case 'Edit':
      return (
        String(toolInput.file_path ?? toolInput.filePath ?? '')
          .split('/')
          .pop() ?? ''
      )
    case 'Bash':
      return String(toolInput.command ?? '').slice(0, 60)
    case 'Grep':
      return String(toolInput.pattern ?? '').slice(0, 40)
    case 'Glob':
      return String(toolInput.pattern ?? '').slice(0, 40)
    default:
      return execution.summary ?? ''
  }
}

function StatusIcon({ status }: { status: ToolExecution['status'] }) {
  switch (status) {
    case 'running':
      return <Loader2 className="w-3 h-3 text-blue-500 dark:text-blue-400 animate-spin" />
    case 'complete':
      return <Check className="w-3 h-3 text-green-500 dark:text-green-400" />
    case 'error':
      return <X className="w-3 h-3 text-red-500 dark:text-red-400" />
  }
}

/** Extract a concise error reason from tool result (first meaningful line). */
function getErrorReason(execution: ToolExecution): string | undefined {
  if (execution.status !== 'error') return undefined
  if (!execution.result?.output) return undefined

  const firstLine = execution.result.output.split('\n').filter(Boolean)[0]
  return firstLine?.slice(0, 120) || undefined
}

export function ToolChip({ execution }: ToolChipProps) {
  const preview = getToolPreview(execution)
  const errorReason = getErrorReason(execution)

  return (
    <div className="space-y-0.5">
      <div className="inline-flex items-center gap-1.5 px-2 py-1 rounded bg-gray-50 dark:bg-gray-800/50 border border-gray-200/50 dark:border-gray-700/50 text-xs">
        <Wrench className="w-3 h-3 text-gray-400 dark:text-gray-500 flex-shrink-0" />
        <span className="font-mono font-medium text-gray-700 dark:text-gray-300">
          {execution.toolName}
        </span>
        {preview && (
          <span className="text-gray-500 dark:text-gray-400 truncate max-w-[200px]">{preview}</span>
        )}
        <StatusIcon status={execution.status} />
      </div>

      {errorReason && (
        <div className="px-2 text-[11px] font-mono text-red-500 dark:text-red-400 truncate max-w-md">
          {errorReason}
        </div>
      )}
    </div>
  )
}
