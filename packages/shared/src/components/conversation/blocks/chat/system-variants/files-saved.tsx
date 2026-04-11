import type { FilesSaved } from '../../../../../types/sidecar-protocol'
import { Save } from 'lucide-react'

interface Props {
  data: FilesSaved
}

export function FilesSavedPill({ data }: Props) {
  const saved = data?.files?.length ?? 0
  const failed = data?.failed?.length ?? 0
  const label = `${saved} saved${failed > 0 ? `, ${failed} failed` : ''}`
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <Save className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{label}</span>
    </div>
  )
}
