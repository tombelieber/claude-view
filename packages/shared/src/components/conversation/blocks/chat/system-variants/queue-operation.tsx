import type { QueueOperation } from '../../../../../types/sidecar-protocol'
import { Clock } from 'lucide-react'

interface Props {
  data: QueueOperation
}

/**
 * Render user-typed enqueue as a chat bubble (right-aligned, like user messages).
 *
 * Only `enqueue` with non-empty content is meaningful to a chat viewer — dequeue /
 * remove / popAll are backend lifecycle events with no user-facing content, so this
 * renderer returns `null` for them. This was previously a registry-level
 * `canRender` gate; moving it here keeps per-variant visibility decisions local to
 * the renderer (see chat/registry.ts for the rationale).
 */
export function QueueOperationBubble({ data }: Props) {
  if (data.operation !== 'enqueue') return null
  if (!data.content?.trim()) return null

  return (
    <div data-testid="queued-user-message" className="flex justify-end">
      <div className="max-w-[80%]">
        <div className="px-3.5 py-2.5 rounded-2xl rounded-br-md bg-blue-500/80 dark:bg-blue-600/80 text-white">
          <p className="text-sm whitespace-pre-wrap break-words">{data.content}</p>
        </div>
        <div className="flex items-center justify-end gap-1 mt-1 px-1">
          <Clock className="w-2.5 h-2.5 text-gray-400 dark:text-gray-500" />
          <span className="text-xs text-gray-400 dark:text-gray-500">Queued</span>
        </div>
      </div>
    </div>
  )
}
