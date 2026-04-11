import { Timer } from 'lucide-react'

interface Props {
  data: Record<string, unknown>
}

export function ScheduledTaskFirePill({ data }: Props) {
  const content = (data?.content as string) || 'Scheduled task fired'
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Timer className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{content}</span>
    </div>
  )
}
