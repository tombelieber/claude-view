import type { StreamDelta } from '../../../../../types/sidecar-protocol'
import { Zap } from 'lucide-react'

interface Props {
  data: StreamDelta
}

export function StreamDeltaPill({ data }: Props) {
  const shortId = data?.messageId?.slice(0, 8) ?? ''
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-400 dark:text-gray-500">
      <Zap className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">stream_delta [{shortId}]</span>
    </div>
  )
}
