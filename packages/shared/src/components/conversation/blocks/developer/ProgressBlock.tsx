import { AgentProgressCard } from '../../../AgentProgressCard'
import { BashProgressCard } from '../../../BashProgressCard'
import { HookProgressCard } from '../../../HookProgressCard'
import { McpProgressCard } from '../../../McpProgressCard'
import { QueryProgressCard } from '../../../QueryProgressCard'
import { SearchProgressCard } from '../../../SearchProgressCard'
import { TaskQueueCard } from '../../../TaskQueueCard'
import type { ProgressBlock as ProgressBlockType } from '../../../../types/blocks'
import { cn } from '../../../../utils/cn'
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

/** Extract a short label for the EventCard header — type-safe via discriminated union. */
function extractLabel(block: ProgressBlockType): string | undefined {
  const { data } = block
  switch (data.type) {
    case 'bash':
      return undefined // card renders its own stats
    case 'agent':
      return data.prompt?.slice(0, 60) || undefined
    case 'hook':
      return `${data.hookEvent} \u2192 ${data.hookName}`
    case 'mcp':
      return `${data.serverName}:${data.toolName}`
    case 'task_queue':
      return data.taskDescription || undefined
    case 'search':
      return `${data.query} (${data.resultCount} results)`
    case 'query':
      return data.query || undefined
    default:
      return undefined
  }
}

/** Render the purpose-built card for each variant — type-safe, no phantom props. */
function renderCard(block: ProgressBlockType): React.ReactNode {
  const { data } = block
  switch (data.type) {
    case 'bash':
      return (
        <BashProgressCard
          output={data.output}
          fullOutput={data.fullOutput}
          elapsedTimeSeconds={data.elapsedTimeSeconds}
          totalLines={data.totalLines}
          totalBytes={data.totalBytes}
          taskId={data.taskId}
          blockId={block.id}
        />
      )
    case 'agent':
      return (
        <AgentProgressCard
          agentId={data.agentId}
          prompt={data.prompt}
          message={data.message}
          blockId={block.id}
        />
      )
    case 'hook':
      return (
        <HookProgressCard
          hookEvent={data.hookEvent}
          hookName={data.hookName}
          command={data.command}
          statusMessage={data.statusMessage}
          blockId={block.id}
        />
      )
    case 'mcp':
      return (
        <McpProgressCard
          serverName={data.serverName}
          toolName={data.toolName}
          status={data.status}
        />
      )
    case 'task_queue':
      return <TaskQueueCard taskDescription={data.taskDescription} taskType={data.taskType} />
    case 'search':
      return <SearchProgressCard query={data.query} resultCount={data.resultCount} />
    case 'query':
      return <QueryProgressCard query={data.query} blockId={block.id} />
    default:
      return null
  }
}

export function DevProgressBlock({ block }: ProgressBlockProps) {
  const { data } = block
  const config = VARIANT_CONFIG[block.variant] ?? { dot: 'gray' as const, chip: block.variant }
  const label = extractLabel(block)
  const elapsed = data.type === 'bash' ? data.elapsedTimeSeconds : undefined

  const meta = (
    <div className="flex items-center gap-2 flex-shrink-0">
      {block.parentToolUseId && (
        <span className="text-[9px] font-mono text-blue-600 dark:text-blue-400 bg-blue-500/10 px-1.5 py-0.5 rounded">
          {block.parentToolUseId.slice(0, 12)}
        </span>
      )}
      {elapsed != null && (
        <span
          className={cn(
            'text-[9px] font-mono tabular-nums px-1.5 py-0.5 rounded',
            elapsed > 30
              ? 'text-red-600 dark:text-red-400 bg-red-500/10'
              : elapsed > 5
                ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10'
                : 'text-gray-500 dark:text-gray-400 bg-gray-500/10',
          )}
        >
          {elapsed.toFixed(1)}s
        </span>
      )}
    </div>
  )

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
      {renderCard(block)}
    </EventCard>
  )
}
