import type { Informational } from '../../../../../types/sidecar-protocol'
import { Info } from 'lucide-react'

interface Props {
  data: Informational
}

export function InformationalBlock({ data }: Props) {
  const text = data?.content || data?.message || ''
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Info className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{text}</span>
    </div>
  )
}
