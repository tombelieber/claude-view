import { GitBranch } from 'lucide-react'
import { useCompactCodeBlock } from '../contexts/CodeRenderContext'

/**
 * HookProgressCard — purpose-built for HookProgress schema.
 *
 * Schema fields: hookEvent, hookName, command, statusMessage
 * Every field is rendered. No phantom props.
 */

interface HookProgressCardProps {
  /** Lifecycle event that triggered this hook (e.g. "PreToolUse", "PostToolUse") */
  hookEvent: string
  /** Hook identifier */
  hookName: string
  /** Shell command / script path being executed */
  command: string
  /** Human-readable status text from the hook */
  statusMessage: string
  /** UI-only: stable key for code block rendering */
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
    <div className="py-0.5 border-l-2 border-l-amber-400 pl-1 my-1">
      {/* Event → Hook flow */}
      <div className="flex items-center gap-1.5 mb-0.5">
        <GitBranch className="w-3 h-3 text-amber-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-amber-400 dark:text-amber-500 flex-shrink-0">
          {hookEvent}
        </span>
        <span className="text-[10px] text-gray-600 dark:text-gray-600" aria-hidden="true">
          {'\u2192'}
        </span>
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 truncate">
          {hookName}
        </span>
      </div>

      {/* Command script */}
      <CompactCodeBlock
        code={command}
        language="bash"
        blockId={blockId ? `${blockId}-cmd` : `hook-${hookName}-cmd`}
      />

      {/* Status message */}
      {statusMessage && (
        <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 mt-0.5 px-1">
          {statusMessage}
        </div>
      )}
    </div>
  )
}
