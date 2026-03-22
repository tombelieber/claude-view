import { Plug } from 'lucide-react'
import { cn } from '../utils/cn'

/**
 * McpProgressCard — purpose-built for McpProgress schema.
 *
 * Schema fields: serverName, toolName, status
 * Every field is rendered. No phantom props.
 */

interface McpProgressCardProps {
  /** MCP server name */
  serverName: string
  /** Tool/method being invoked */
  toolName: string
  /** Execution status (e.g. "running", "completed", "error") */
  status: string
}

const STATUS_STYLE: Record<string, string> = {
  running: 'text-blue-400 bg-blue-500/10 dark:bg-blue-500/20',
  completed: 'text-green-400 bg-green-500/10 dark:bg-green-500/20',
  error: 'text-red-400 bg-red-500/10 dark:bg-red-500/20',
}

export function McpProgressCard({ serverName, toolName, status }: McpProgressCardProps) {
  const statusStyle = STATUS_STYLE[status] ?? 'text-gray-400 bg-gray-500/10 dark:bg-gray-500/20'
  const isRunning = status === 'running'

  return (
    <div className="py-0.5 border-l-2 border-l-blue-400 pl-1 my-1" aria-label="MCP tool call">
      <div className="flex items-center gap-1.5">
        <Plug className="w-3 h-3 text-blue-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate flex-1">
          {serverName}.{toolName}
        </span>
        <span
          className={cn(
            'inline-flex items-center gap-1 text-[10px] font-mono px-1.5 py-0.5 rounded flex-shrink-0',
            statusStyle,
          )}
        >
          {isRunning && <span className="w-1.5 h-1.5 rounded-full bg-blue-400 animate-pulse" />}
          {status}
        </span>
      </div>
    </div>
  )
}
