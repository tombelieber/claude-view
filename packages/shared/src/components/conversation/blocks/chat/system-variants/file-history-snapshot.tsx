import type { FileHistorySnapshot } from '../../../../../types/sidecar-protocol'
import { History } from 'lucide-react'

interface Props {
  data: FileHistorySnapshot
}

export function FileHistorySnapshotPill({ data }: Props) {
  const fileCount =
    data?.files?.length ??
    Object.keys(data?.snapshot?.trackedFileBackups ?? {}).length ??
    data?.fileCount ??
    0
  const label = `${fileCount} file(s) snapshot${data?.isSnapshotUpdate ? ' (update)' : ''}`
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <History className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">{label}</span>
    </div>
  )
}
