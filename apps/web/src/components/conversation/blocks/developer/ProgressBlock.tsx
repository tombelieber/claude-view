import { AgentProgressCard } from '@claude-view/shared/components/AgentProgressCard'
import { BashProgressCard } from '@claude-view/shared/components/BashProgressCard'
import { HookProgressCard } from '@claude-view/shared/components/HookProgressCard'
import { McpProgressCard } from '@claude-view/shared/components/McpProgressCard'
import { TaskQueueCard } from '@claude-view/shared/components/TaskQueueCard'
import type { ProgressBlock as ProgressBlockType } from '@claude-view/shared/types/blocks'
import { cn } from '../../../../lib/utils'
import { EventCard } from './EventCard'

interface ProgressBlockProps {
  block: ProgressBlockType
}

const VARIANT_CONFIG: Record<
  string,
  {
    dot: 'gray' | 'indigo' | 'amber' | 'blue' | 'orange' | 'cyan' | 'teal'
    chip: string
    chipColor?: string
  }
> = {
  bash: {
    dot: 'gray',
    chip: 'Bash',
    chipColor: 'bg-gray-500/10 dark:bg-gray-500/20 text-gray-700 dark:text-gray-300',
  },
  agent: {
    dot: 'indigo',
    chip: 'Agent',
    chipColor: 'bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300',
  },
  hook: {
    dot: 'amber',
    chip: 'Hook',
    chipColor: 'bg-amber-500/10 dark:bg-amber-500/20 text-amber-700 dark:text-amber-300',
  },
  mcp: {
    dot: 'blue',
    chip: 'MCP',
    chipColor: 'bg-blue-500/10 dark:bg-blue-500/20 text-blue-700 dark:text-blue-300',
  },
  task_queue: {
    dot: 'orange',
    chip: 'Queue',
    chipColor: 'bg-orange-500/10 dark:bg-orange-500/20 text-orange-700 dark:text-orange-300',
  },
  search: {
    dot: 'cyan',
    chip: 'Search',
    chipColor: 'bg-cyan-500/10 dark:bg-cyan-500/20 text-cyan-700 dark:text-cyan-300',
  },
  query: {
    dot: 'teal',
    chip: 'Query',
    chipColor: 'bg-teal-500/10 dark:bg-teal-500/20 text-teal-700 dark:text-teal-300',
  },
}

function extractLabel(variant: string, data: Record<string, unknown>): string | undefined {
  switch (variant) {
    case 'bash':
      return undefined // card shows it
    case 'agent':
      return (data.prompt as string)?.slice(0, 60) || undefined
    case 'hook':
      return `${data.hookEvent ?? ''} → ${data.hookName ?? ''}`
    case 'mcp':
      return `${data.serverName ?? ''}:${data.toolName ?? ''}`
    case 'task_queue':
      return (data.taskDescription as string) || undefined
    case 'search':
      return `${data.query ?? ''} (${data.resultCount ?? 0} results)`
    case 'query':
      return (data.query as string) || undefined
    default:
      return undefined
  }
}

export function DevProgressBlock({ block }: ProgressBlockProps) {
  const d = block.data as Record<string, unknown>
  const config = VARIANT_CONFIG[block.variant] ?? { dot: 'gray' as const, chip: block.variant }
  const label = extractLabel(block.variant, d)
  const elapsed = typeof d.elapsedTimeSeconds === 'number' ? d.elapsedTimeSeconds : undefined

  const meta = (
    <div className="flex items-center gap-2 flex-shrink-0">
      {block.parentToolUseId && (
        <span className="text-[9px] font-mono text-blue-400 dark:text-blue-500 bg-blue-500/10 px-1.5 py-0.5 rounded">
          {block.parentToolUseId.slice(0, 12)}
        </span>
      )}
      {elapsed != null && (
        <span
          className={cn(
            'text-[9px] font-mono tabular-nums px-1.5 py-0.5 rounded',
            elapsed > 30
              ? 'text-red-400 bg-red-500/10'
              : elapsed > 5
                ? 'text-amber-400 bg-amber-500/10'
                : 'text-gray-400 bg-gray-500/10',
          )}
        >
          {elapsed.toFixed(1)}s
        </span>
      )}
    </div>
  )

  const card = (() => {
    switch (block.variant) {
      case 'bash':
        return (
          <BashProgressCard
            command={String(d.output ?? d.fullOutput ?? '')}
            output={String(d.fullOutput ?? d.output ?? '')}
            duration={elapsed != null ? elapsed * 1000 : undefined}
          />
        )
      case 'agent':
        return (
          <AgentProgressCard agentId={String(d.agentId ?? '')} prompt={String(d.prompt ?? '')} />
        )
      case 'hook':
        return (
          <HookProgressCard
            hookEvent={String(d.hookEvent ?? '')}
            hookName={String(d.hookName ?? '')}
            command={String(d.command ?? '')}
          />
        )
      case 'mcp':
        return (
          <McpProgressCard server={String(d.serverName ?? '')} method={String(d.toolName ?? '')} />
        )
      case 'task_queue':
        return <TaskQueueCard />
      default:
        return null
    }
  })()

  return (
    <EventCard
      dot={config.dot}
      chip={config.chip}
      chipColor={config.chipColor}
      label={label}
      meta={meta}
      pulse
      rawData={block}
    >
      {card}
    </EventCard>
  )
}
