import type { AiTitle } from '../../../../../types/sidecar-protocol'
import { Sparkles } from 'lucide-react'

interface Props {
  data: AiTitle
}

export function AiTitlePill({ data }: Props) {
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Sparkles className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{data?.aiTitle}</span>
    </div>
  )
}
