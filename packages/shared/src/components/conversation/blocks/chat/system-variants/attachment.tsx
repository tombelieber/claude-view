import { GitBranch, Paperclip } from 'lucide-react'
import { CollapsibleJson } from '../../shared/CollapsibleJson'
import { StatusBadge } from '../../shared/StatusBadge'

interface AttachmentPillProps {
  data: Record<string, unknown>
}

export function AttachmentPill({ data }: AttachmentPillProps) {
  const att = (data?.attachment as Record<string, unknown>) ?? null

  if (!att) {
    return (
      <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
        <Paperclip className="w-3 h-3 flex-shrink-0" />
        <StatusBadge label="attachment" color="gray" />
      </div>
    )
  }

  const attType = (att.type as string) ?? 'unknown'

  // Tier 1 — async_hook_response (10K+ occurrences)
  if (attType === 'async_hook_response') {
    const hookName = att.hookName as string | undefined
    const hookEvent = att.hookEvent as string | undefined
    const exitCode = att.exitCode as number | undefined
    return (
      <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
        <GitBranch className="w-3 h-3 flex-shrink-0" />
        <span className="truncate">{hookName ?? 'hook'}</span>
        {hookEvent && <StatusBadge label={hookEvent} color="amber" />}
        {exitCode != null && exitCode !== 0 && (
          <StatusBadge label={`exit ${exitCode}`} color="red" />
        )}
      </div>
    )
  }

  // Tier 2 — file
  if (attType === 'file') {
    const addedNames = (att.addedNames ?? []) as string[]
    const removedNames = (att.removedNames ?? []) as string[]
    const addedLines = att.addedLines as number | undefined
    return (
      <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
        <Paperclip className="w-3 h-3 flex-shrink-0" />
        <span>
          {addedNames.length} added, {removedNames.length} removed
        </span>
        {addedLines != null && (
          <span className="font-mono text-gray-400 dark:text-gray-500">+{addedLines} lines</span>
        )}
      </div>
    )
  }

  // Tier 3 — all other types (generic)
  return (
    <div className="space-y-1 px-3 py-1">
      <div className="flex items-center gap-2 text-xs">
        <Paperclip className="w-3 h-3 text-gray-400 dark:text-gray-500" />
        <StatusBadge label={attType} color="blue" />
      </div>
      <CollapsibleJson data={att} label="Attachment" />
    </div>
  )
}
