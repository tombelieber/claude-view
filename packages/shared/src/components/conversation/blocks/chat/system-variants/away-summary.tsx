import { Coffee } from 'lucide-react'

interface Props {
  data: Record<string, unknown>
}

export function AwaySummaryPill({ data }: Props) {
  const content = (data?.content as string) || 'Auto-recap on return'
  return (
    <div className="flex items-start gap-2 px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
      <Coffee className="w-3 h-3 mt-0.5 flex-shrink-0" />
      <span className="leading-relaxed">{content}</span>
    </div>
  )
}
