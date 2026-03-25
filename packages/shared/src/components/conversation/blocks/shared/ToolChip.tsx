import type { ToolExecution } from '../../../../types/blocks'
import {
  Bot,
  Check,
  FileText,
  FilePlus,
  Files,
  Globe,
  Loader2,
  Pencil,
  Plug,
  Search,
  Terminal,
  Wrench,
  X,
} from 'lucide-react'
import { cn } from '../../../../utils/cn'

interface ToolChipProps {
  execution: ToolExecution
}

// ── Tool visual identity ──────────────────────────────────────────────

type ToolStyle = {
  icon: React.ElementType
  accent: string // icon color
  bg: string // chip background
  border: string // chip border
}

const TOOL_STYLES: Record<string, ToolStyle> = {
  Read: {
    icon: FileText,
    accent: 'text-blue-500 dark:text-blue-400',
    bg: 'bg-blue-50 dark:bg-blue-900/20',
    border: 'border-blue-200/50 dark:border-blue-700/40',
  },
  Write: {
    icon: FilePlus,
    accent: 'text-amber-500 dark:text-amber-400',
    bg: 'bg-amber-50 dark:bg-amber-900/20',
    border: 'border-amber-200/50 dark:border-amber-700/40',
  },
  Edit: {
    icon: Pencil,
    accent: 'text-amber-500 dark:text-amber-400',
    bg: 'bg-amber-50 dark:bg-amber-900/20',
    border: 'border-amber-200/50 dark:border-amber-700/40',
  },
  Bash: {
    icon: Terminal,
    accent: 'text-green-500 dark:text-green-400',
    bg: 'bg-green-50 dark:bg-green-900/20',
    border: 'border-green-200/50 dark:border-green-700/40',
  },
  Grep: {
    icon: Search,
    accent: 'text-violet-500 dark:text-violet-400',
    bg: 'bg-violet-50 dark:bg-violet-900/20',
    border: 'border-violet-200/50 dark:border-violet-700/40',
  },
  Glob: {
    icon: Files,
    accent: 'text-violet-500 dark:text-violet-400',
    bg: 'bg-violet-50 dark:bg-violet-900/20',
    border: 'border-violet-200/50 dark:border-violet-700/40',
  },
  Agent: {
    icon: Bot,
    accent: 'text-indigo-500 dark:text-indigo-400',
    bg: 'bg-indigo-50 dark:bg-indigo-900/20',
    border: 'border-indigo-200/50 dark:border-indigo-700/40',
  },
  WebSearch: {
    icon: Globe,
    accent: 'text-cyan-500 dark:text-cyan-400',
    bg: 'bg-cyan-50 dark:bg-cyan-900/20',
    border: 'border-cyan-200/50 dark:border-cyan-700/40',
  },
  WebFetch: {
    icon: Globe,
    accent: 'text-cyan-500 dark:text-cyan-400',
    bg: 'bg-cyan-50 dark:bg-cyan-900/20',
    border: 'border-cyan-200/50 dark:border-cyan-700/40',
  },
}

const FALLBACK_STYLE: ToolStyle = {
  icon: Wrench,
  accent: 'text-gray-400 dark:text-gray-500',
  bg: 'bg-gray-50 dark:bg-gray-800/50',
  border: 'border-gray-200/50 dark:border-gray-700/50',
}

/** Resolve style — check tool name first, then fall back to category-based for MCP/Agent. */
function getToolStyle(execution: ToolExecution): ToolStyle {
  if (TOOL_STYLES[execution.toolName]) return TOOL_STYLES[execution.toolName]
  // MCP tools not in the map get a teal Plug style
  if (execution.category === 'mcp')
    return {
      icon: Plug,
      accent: 'text-teal-500 dark:text-teal-400',
      bg: 'bg-teal-50 dark:bg-teal-900/20',
      border: 'border-teal-200/50 dark:border-teal-700/40',
    }
  if (execution.category === 'agent') return TOOL_STYLES.Agent
  return FALLBACK_STYLE
}

// ── Preview text (what the tool is operating on) ──────────────────────

function getToolPreview(execution: ToolExecution): string {
  const { toolName, toolInput } = execution
  switch (toolName) {
    case 'Read':
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

// ── Compact result hint (what happened) ───────────────────────────────

function getResultHint(execution: ToolExecution): string | undefined {
  if (execution.status !== 'complete') return undefined
  const output = execution.result?.output
  if (!output) return undefined

  switch (execution.toolName) {
    case 'Read': {
      const lineCount = output.split('\n').length
      return lineCount > 1 ? `${lineCount} lines` : undefined
    }
    case 'Bash': {
      // Show last meaningful line as a quick hint
      const lines = output.split('\n').filter(Boolean)
      const last = lines[lines.length - 1]
      if (!last || last.length > 50) return undefined
      return last
    }
    case 'Grep': {
      const fileCount = output.split('\n').filter(Boolean).length
      return fileCount > 0 ? `${fileCount} ${fileCount === 1 ? 'match' : 'matches'}` : undefined
    }
    case 'Glob': {
      const matchCount = output.split('\n').filter(Boolean).length
      return matchCount > 0 ? `${matchCount} ${matchCount === 1 ? 'file' : 'files'}` : undefined
    }
    default:
      return undefined
  }
}

// ── Duration formatting ───────────────────────────────────────────────

function formatDuration(ms: number): string | undefined {
  if (ms < 500) return undefined // too short to show
  if (ms < 1000) return `${ms}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60_000)}m${Math.round((ms % 60_000) / 1000)}s`
}

// ── Status icon ───────────────────────────────────────────────────────

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

// ── Error reason ──────────────────────────────────────────────────────

function getErrorReason(execution: ToolExecution): string | undefined {
  if (execution.status !== 'error') return undefined
  if (!execution.result?.output) return undefined
  const firstLine = execution.result.output.split('\n').filter(Boolean)[0]
  return firstLine?.slice(0, 120) || undefined
}

// ── ToolChip ──────────────────────────────────────────────────────────

export function ToolChip({ execution }: ToolChipProps) {
  const style = getToolStyle(execution)
  const Icon = style.icon
  const preview = getToolPreview(execution)
  const errorReason = getErrorReason(execution)
  const resultHint = getResultHint(execution)
  const durationLabel = execution.duration ? formatDuration(execution.duration) : undefined

  return (
    <div className="space-y-0.5">
      <div
        className={cn(
          'inline-flex items-center gap-1.5 px-2 py-1 rounded border text-xs',
          style.bg,
          style.border,
        )}
      >
        <Icon className={cn('w-3 h-3 flex-shrink-0', style.accent)} />
        <span className="font-mono font-medium text-gray-700 dark:text-gray-300">
          {execution.toolName}
        </span>
        {preview && (
          <span className="text-gray-500 dark:text-gray-400 truncate max-w-[200px]">{preview}</span>
        )}
        <StatusIcon status={execution.status} />
        {resultHint && (
          <span className="text-xs text-gray-400 dark:text-gray-500 font-mono truncate max-w-[120px]">
            {resultHint}
          </span>
        )}
        {durationLabel && (
          <span className="text-xs text-gray-400 dark:text-gray-500 font-mono tabular-nums">
            {durationLabel}
          </span>
        )}
      </div>

      {errorReason && (
        <div className="px-2 text-xs font-mono text-red-500 dark:text-red-400 truncate max-w-md">
          {errorReason}
        </div>
      )}
    </div>
  )
}
