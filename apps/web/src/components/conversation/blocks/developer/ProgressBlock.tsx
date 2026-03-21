import { AgentProgressCard } from '@claude-view/shared/components/AgentProgressCard'
import { BashProgressCard } from '@claude-view/shared/components/BashProgressCard'
import { HookProgressCard } from '@claude-view/shared/components/HookProgressCard'
import { McpProgressCard } from '@claude-view/shared/components/McpProgressCard'
import { TaskQueueCard } from '@claude-view/shared/components/TaskQueueCard'
import type { ProgressBlock as ProgressBlockType } from '@claude-view/shared/types/blocks'
import { Activity } from 'lucide-react'

interface ProgressBlockProps {
  block: ProgressBlockType
}

/** Developer mode: reuses polished shared cards from the RichPane pipeline. */
export function DevProgressBlock({ block }: ProgressBlockProps) {
  const d = block.data as Record<string, unknown>

  return (
    <div className="text-xs">
      {/* Category + parentToolUseId badge header */}
      <div className="flex items-center gap-2 mb-1">
        <span className="rounded bg-gray-200 dark:bg-gray-700 px-1.5 py-0.5 text-[10px] text-gray-500 dark:text-gray-400">
          {block.category}
        </span>
        {block.parentToolUseId && (
          <span className="rounded bg-blue-100 dark:bg-blue-900/30 px-1.5 py-0.5 text-[10px] text-blue-600 dark:text-blue-400 font-mono">
            {block.parentToolUseId}
          </span>
        )}
      </div>

      {/* Variant-specific card — reuse shared polished components */}
      {block.variant === 'bash' && (
        <BashProgressCard
          command={String(d.output ?? d.fullOutput ?? '')}
          output={String(d.fullOutput ?? d.output ?? '')}
          duration={
            typeof d.elapsedTimeSeconds === 'number' ? d.elapsedTimeSeconds * 1000 : undefined
          }
        />
      )}
      {block.variant === 'agent' && (
        <AgentProgressCard agentId={String(d.agentId ?? '')} prompt={String(d.prompt ?? '')} />
      )}
      {block.variant === 'hook' && (
        <HookProgressCard
          hookEvent={String(d.hookEvent ?? '')}
          hookName={String(d.hookName ?? '')}
          command={String(d.command ?? '')}
        />
      )}
      {block.variant === 'mcp' && (
        <McpProgressCard server={String(d.serverName ?? '')} method={String(d.toolName ?? '')} />
      )}
      {block.variant === 'task_queue' && <TaskQueueCard />}
      {block.variant === 'search' && (
        <div className="flex items-center gap-2 px-3 py-1 text-gray-500 dark:text-gray-400">
          <Activity className="w-3 h-3" />
          <span>
            Search: {String(d.query ?? '')} ({String(d.resultCount ?? 0)} results)
          </span>
        </div>
      )}
      {block.variant === 'query' && (
        <div className="flex items-center gap-2 px-3 py-1 text-gray-500 dark:text-gray-400">
          <Activity className="w-3 h-3" />
          <span className="font-mono">{String(d.query ?? '')}</span>
        </div>
      )}
    </div>
  )
}
