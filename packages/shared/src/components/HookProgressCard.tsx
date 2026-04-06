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
  hookName,
  command,
  statusMessage,
  blockId,
}: HookProgressCardProps) {
  const CompactCodeBlock = useCompactCodeBlock()

  return (
    <div className="space-y-1">
      {/* Status message — event→name already shown in EventCard header */}
      {statusMessage && (
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400">{statusMessage}</span>
      )}

      {/* Command — only when non-empty */}
      {command && (
        <CompactCodeBlock
          code={command}
          language="bash"
          blockId={blockId ? `${blockId}-cmd` : `hook-${hookName}-cmd`}
        />
      )}
    </div>
  )
}
