import { Tag } from 'lucide-react'

interface Props {
  data: Record<string, unknown>
}

export function CustomTitlePill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400">
      <Tag className="w-3 h-3 flex-shrink-0" />
      <span className="font-medium truncate">{data.customTitle as string}</span>
    </div>
  )
}
