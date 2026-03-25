import { GitBranch } from 'lucide-react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * HookProgressCard — follows HookEventDetail pattern.
 * Header with event→name + status, command as code block.
 *
 * Schema: hookEvent, hookName, command, statusMessage
 */

interface HookProgressCardProps {
  hookEvent: string
  hookName: string
  command: string
  statusMessage: string
  blockId?: string
}

export function HookProgressCard({
  hookEvent,
  hookName,
  command,
  statusMessage,
  blockId,
}: HookProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()

  return (
    <div className="space-y-1">
      {/* Header: event → name + status */}
      <div className="flex items-center gap-2 text-xs font-mono">
        <GitBranch className="w-3 h-3 text-amber-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-amber-700 dark:text-amber-300">{hookEvent}</span>
        <span className="text-gray-500 dark:text-gray-500" aria-hidden="true">
          {'\u2192'}
        </span>
        <span className="text-gray-700 dark:text-gray-300 truncate">{hookName}</span>
        {statusMessage && (
          <span className="text-gray-500 dark:text-gray-400 truncate ml-auto flex-shrink-0">
            {statusMessage}
          </span>
        )}
      </div>

      {/* Command — always visible */}
      <CompactCodeBlock
        code={command}
        language="bash"
        blockId={blockId ? `${blockId}-cmd` : `hook-${hookName}-cmd`}
      />
    </div>
  )
}
