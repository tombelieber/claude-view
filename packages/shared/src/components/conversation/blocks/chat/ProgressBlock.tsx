import type { ProgressBlock as ProgressBlockType } from '../../../../types/blocks'
import { Bot, Clock, Database, GitBranch, Plug, Search, Terminal } from 'lucide-react'
import { cn } from '../../../../utils/cn'

interface ProgressBlockProps {
  block: ProgressBlockType
}

const VARIANT_STYLE: Record<string, { icon: React.ElementType; accent: string; dotColor: string }> =
  {
    bash: {
      icon: Terminal,
      accent: 'text-gray-500 dark:text-gray-400',
      dotColor: 'bg-gray-400',
    },
    agent: {
      icon: Bot,
      accent: 'text-indigo-500 dark:text-indigo-400',
      dotColor: 'bg-indigo-500',
    },
    hook: {
      icon: GitBranch,
      accent: 'text-amber-500 dark:text-amber-400',
      dotColor: 'bg-amber-400',
    },
    mcp: {
      icon: Plug,
      accent: 'text-blue-500 dark:text-blue-400',
      dotColor: 'bg-blue-500',
    },
    task_queue: {
      icon: Clock,
      accent: 'text-orange-500 dark:text-orange-400',
      dotColor: 'bg-orange-500',
    },
    search: {
      icon: Search,
      accent: 'text-cyan-500 dark:text-cyan-400',
      dotColor: 'bg-cyan-500',
    },
    query: {
      icon: Database,
      accent: 'text-teal-500 dark:text-teal-400',
      dotColor: 'bg-teal-500',
    },
  }

const FALLBACK_STYLE = {
  icon: Terminal,
  accent: 'text-gray-500 dark:text-gray-400',
  dotColor: 'bg-gray-400',
}

/** Extract a human-readable activity label from an agent_progress message payload. */
function extractAgentActivity(message: unknown): { label: string; detail?: string } | null {
  if (!message || typeof message !== 'object') return null
  const msg = message as Record<string, unknown>
  const inner = msg.message as Record<string, unknown> | undefined
  if (!inner) return null

  const content = inner.content
  if (!Array.isArray(content)) return null

  // Assistant messages → tool_use calls (agent is calling a tool)
  if (msg.type === 'assistant') {
    const toolNames: string[] = []
    for (const c of content) {
      if (c && typeof c === 'object' && (c as Record<string, unknown>).type === 'tool_use') {
        const name = (c as Record<string, unknown>).name
        if (typeof name === 'string') toolNames.push(name)
      }
    }
    if (toolNames.length > 0) {
      return {
        label: `Agent → ${toolNames.join(', ')}`,
      }
    }
  }

  // User messages with tool_result → agent received results
  if (msg.type === 'user') {
    const resultIds: string[] = []
    let hasText = false
    for (const c of content) {
      if (!c || typeof c !== 'object') continue
      const ct = (c as Record<string, unknown>).type
      if (ct === 'tool_result') resultIds.push('result')
      if (ct === 'text') hasText = true
    }
    if (resultIds.length > 0) {
      return {
        label: `Agent ← ${resultIds.length} result${resultIds.length > 1 ? 's' : ''}`,
      }
    }
    // First user message with text = the agent's prompt
    if (hasText) {
      const textBlock = content.find(
        (c: unknown) =>
          c && typeof c === 'object' && (c as Record<string, unknown>).type === 'text',
      ) as Record<string, unknown> | undefined
      const text = typeof textBlock?.text === 'string' ? textBlock.text : ''
      if (text) {
        return {
          label: `Agent: ${text.slice(0, 80)}${text.length > 80 ? '…' : ''}`,
        }
      }
    }
  }

  return null
}

/** Type-safe extraction via discriminated union — every schema field accessible. */
function extractInfo(block: ProgressBlockType): {
  label: string
  detail?: string
  isError?: boolean
} {
  const { data } = block
  switch (data.type) {
    case 'bash': {
      const lastLine = data.output.split('\n').filter(Boolean).pop()
      return {
        label: data.elapsedTimeSeconds
          ? `Running\u2026 ${data.elapsedTimeSeconds.toFixed(1)}s`
          : 'Running\u2026',
        detail: lastLine || undefined,
      }
    }
    case 'agent': {
      const activity = extractAgentActivity(data.message)
      if (activity) {
        return {
          label: activity.label,
          detail: activity.detail,
        }
      }
      return {
        label: data.prompt ? `Agent: ${data.prompt}` : 'Agent running\u2026',
      }
    }
    case 'hook':
      return {
        label: data.statusMessage || data.hookName || 'Hook running\u2026',
        detail:
          data.hookEvent && data.hookName ? `${data.hookEvent} \u2192 ${data.hookName}` : undefined,
      }
    case 'mcp': {
      const isError = data.status === 'error'
      return {
        label: data.toolName ? `${data.serverName}/${data.toolName}` : 'MCP call\u2026',
        detail: data.status !== 'running' ? data.status : undefined,
        isError,
      }
    }
    case 'task_queue':
      return {
        label: data.taskDescription || 'Waiting for task\u2026',
        detail: data.taskType || undefined,
      }
    case 'search': {
      return {
        label: data.query || 'Searching\u2026',
        detail:
          data.resultCount !== undefined
            ? `${data.resultCount} ${data.resultCount === 1 ? 'result' : 'results'}`
            : undefined,
      }
    }
    case 'query':
      return {
        label: data.query || 'Running query\u2026',
      }
    default:
      return { label: block.variant }
  }
}

/** Chat mode: compact inline progress indicator with variant-specific styling. */
export function ChatProgressBlock({ block }: ProgressBlockProps) {
  const style = VARIANT_STYLE[block.variant] ?? FALLBACK_STYLE
  const Icon = style.icon
  const { label, detail, isError } = extractInfo(block)

  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs">
      <span
        className={cn(
          'w-1.5 h-1.5 rounded-full flex-shrink-0',
          isError ? 'bg-red-500' : cn(style.dotColor, 'animate-pulse'),
        )}
      />
      <Icon className={cn('w-3.5 h-3.5 flex-shrink-0', isError ? 'text-red-500' : style.accent)} />
      <span className="text-gray-600 dark:text-gray-400 font-mono truncate">{label}</span>
      {detail && (
        <span
          className={cn(
            'font-mono text-xs truncate flex-shrink-0 ml-auto',
            isError ? 'text-red-500 dark:text-red-400' : 'text-gray-500 dark:text-gray-600',
          )}
        >
          {detail}
        </span>
      )}
    </div>
  )
}
