import { Paperclip } from 'lucide-react'
import { StatusBadge } from '../../shared/StatusBadge'

interface Props {
  data: Record<string, unknown>
}

export function AttachmentPill({ data }: Props) {
  const att = (data?.attachment as Record<string, unknown>) ?? {}
  const attType = (att.type as string) ?? 'attachment'
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Paperclip className="w-3 h-3 flex-shrink-0" />
      <StatusBadge label={attType} color="blue" />
    </div>
  )
}
