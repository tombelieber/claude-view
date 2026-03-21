import type { ProgressBlock as ProgressBlockType } from '@claude-view/shared/types/blocks'
import { Activity, Cpu, Play, Terminal, Webhook } from 'lucide-react'

interface ProgressBlockProps {
  block: ProgressBlockType
}

const variantIcon: Record<string, React.ElementType> = {
  bash: Terminal,
  agent: Play,
  mcp: Cpu,
  hook: Webhook,
  task_queue: Activity,
  search: Activity,
  query: Activity,
}

/** Developer mode: rich progress card with variant-specific details. */
export function DevProgressBlock({ block }: ProgressBlockProps) {
  const Icon = variantIcon[block.variant] ?? Activity

  return (
    <div className="rounded-md border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 p-3 text-xs">
      {/* Header */}
      <div className="flex items-center gap-2 mb-2">
        <Icon className="w-3.5 h-3.5 text-gray-500 dark:text-gray-400" />
        <span className="font-medium text-gray-700 dark:text-gray-300 capitalize">
          {block.variant.replace('_', ' ')}
        </span>
        <span className="ml-auto rounded bg-gray-200 dark:bg-gray-700 px-1.5 py-0.5 text-[10px] text-gray-500 dark:text-gray-400">
          {block.category}
        </span>
        {block.parentToolUseId && (
          <span className="rounded bg-blue-100 dark:bg-blue-900/30 px-1.5 py-0.5 text-[10px] text-blue-600 dark:text-blue-400">
            {block.parentToolUseId}
          </span>
        )}
      </div>

      {/* Variant-specific details */}
      {block.variant === 'bash' && <BashDetail data={block.data as Record<string, unknown>} />}
      {block.variant === 'agent' && <AgentDetail data={block.data as Record<string, unknown>} />}
      {block.variant === 'hook' && <HookDetail data={block.data as Record<string, unknown>} />}
      {block.variant === 'mcp' && <McpDetail data={block.data as Record<string, unknown>} />}
      {block.variant === 'task_queue' && (
        <TaskQueueDetail data={block.data as Record<string, unknown>} />
      )}
    </div>
  )
}

function BashDetail({ data }: { data: Record<string, unknown> }) {
  const output = (data.output as string) || (data.fullOutput as string) || ''
  const elapsed = data.elapsedTimeSeconds as number | undefined
  const lines = data.totalLines as number | undefined
  const bytes = data.totalBytes as number | undefined
  return (
    <div className="space-y-1">
      <div className="flex items-center gap-3 text-gray-500 dark:text-gray-400">
        {elapsed != null && <span>{elapsed.toFixed(1)}s</span>}
        {lines != null && <span>{lines} lines</span>}
        {bytes != null && <span>{bytes} bytes</span>}
      </div>
      {output && (
        <pre className="mt-1 max-h-24 overflow-auto rounded bg-gray-900 p-2 text-[11px] text-green-400 font-mono">
          {output}
        </pre>
      )}
    </div>
  )
}

function AgentDetail({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="text-gray-500 dark:text-gray-400">
      <span className="font-mono">{String(data.agentId ?? '—')}</span>
      {data.prompt ? <p className="mt-1 truncate">{String(data.prompt)}</p> : null}
    </div>
  )
}

function HookDetail({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="text-gray-500 dark:text-gray-400 space-y-0.5">
      <div>
        <span className="font-medium">{String(data.hookName ?? '')}</span>
        <span className="ml-2 opacity-70">{String(data.hookEvent ?? '')}</span>
      </div>
      {data.command ? <div className="font-mono text-[11px]">{String(data.command)}</div> : null}
      {data.statusMessage ? <div className="opacity-70">{String(data.statusMessage)}</div> : null}
    </div>
  )
}

function McpDetail({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="text-gray-500 dark:text-gray-400">
      <span className="font-medium">{data.serverName as string}</span>
      <span className="mx-1">/</span>
      <span className="font-mono">{data.toolName as string}</span>
      <span className="ml-2 opacity-70">{data.status as string}</span>
    </div>
  )
}

function TaskQueueDetail({ data }: { data: Record<string, unknown> }) {
  return (
    <div className="text-gray-500 dark:text-gray-400">
      <span>{data.taskDescription as string}</span>
      <span className="ml-2 opacity-70">({data.taskType as string})</span>
    </div>
  )
}
