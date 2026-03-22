import { Plug } from 'lucide-react'
import { cn } from '../utils/cn'

/**
 * McpProgressCard — follows TaskProgressDetail pattern: inline stats.
 *
 * Schema: serverName, toolName, status
 */

interface McpProgressCardProps {
  serverName: string
  toolName: string
  status: string
}

const STATUS_STYLE: Record<string, { text: string; dot: string; bg: string }> = {
  running: {
    text: 'text-blue-700 dark:text-blue-300',
    dot: 'bg-blue-400 animate-pulse',
    bg: 'bg-blue-500/10 dark:bg-blue-500/20',
  },
  completed: {
    text: 'text-green-700 dark:text-green-300',
    dot: 'bg-green-400',
    bg: 'bg-green-500/10 dark:bg-green-500/20',
  },
  error: {
    text: 'text-red-700 dark:text-red-300',
    dot: 'bg-red-400',
    bg: 'bg-red-500/10 dark:bg-red-500/20',
  },
}

const FALLBACK_STATUS = {
  text: 'text-gray-700 dark:text-gray-300',
  dot: 'bg-gray-400',
  bg: 'bg-gray-500/10 dark:bg-gray-500/20',
}

export function McpProgressCard({ serverName, toolName, status }: McpProgressCardProps) {
  const ss = STATUS_STYLE[status] ?? FALLBACK_STATUS

  return (
    <div className="flex items-center gap-3 text-[10px] font-mono" aria-label="MCP tool call">
      <Plug className="w-3 h-3 text-blue-500 flex-shrink-0" aria-hidden="true" />
      <span className="text-gray-700 dark:text-gray-300">{serverName}</span>
      <span className="text-gray-700 dark:text-gray-300">{toolName}</span>
      <span className={cn('inline-flex items-center gap-1 px-1.5 py-0.5 rounded', ss.text, ss.bg)}>
        <span className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', ss.dot)} />
        {status}
      </span>
    </div>
  )
}
