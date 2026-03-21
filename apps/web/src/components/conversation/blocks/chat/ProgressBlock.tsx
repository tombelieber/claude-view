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

/** Chat mode: minimal inline progress indicator. */
export function ChatProgressBlock({ block }: ProgressBlockProps) {
  const Icon = variantIcon[block.variant] ?? Activity

  let label: string
  switch (block.variant) {
    case 'bash': {
      const d = block.data as { output?: string; elapsedTimeSeconds?: number }
      label = d.elapsedTimeSeconds ? `Running… ${d.elapsedTimeSeconds.toFixed(1)}s` : 'Running…'
      break
    }
    case 'agent': {
      const d = block.data as { prompt?: string }
      label = d.prompt ? `Agent: ${d.prompt}` : 'Agent running…'
      break
    }
    case 'hook': {
      const d = block.data as { hookName?: string; statusMessage?: string }
      label = d.statusMessage ?? d.hookName ?? 'Hook running…'
      break
    }
    case 'mcp': {
      const d = block.data as { serverName?: string; toolName?: string }
      label = d.toolName ? `MCP: ${d.serverName}/${d.toolName}` : 'MCP call…'
      break
    }
    default:
      label = block.variant
  }

  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-400 dark:text-gray-500">
      <Icon className="w-3 h-3 flex-shrink-0 animate-pulse" />
      <span className="truncate">{label}</span>
    </div>
  )
}
