import { ListOrdered } from 'lucide-react'
import { cn } from '../lib/utils'

interface MessageQueueEventCardProps {
  operation: 'enqueue' | 'dequeue'
  timestamp: string
  content?: string
  queueId?: string
}

export function MessageQueueEventCard({
  operation,
  timestamp,
  content,
  queueId,
}: MessageQueueEventCardProps) {
  const messageText =
    operation === 'dequeue'
      ? 'Message processed'
      : timestamp
        ? `Message enqueued at ${timestamp}`
        : 'Message enqueued'

  return (
    <div
      className={cn(
        'rounded-lg border border-gray-200 border-l-4 border-l-gray-300 bg-white p-3 my-2'
      )}
      aria-label="Message queue event"
    >
      <div className="flex items-start gap-2">
        <ListOrdered
          className="w-4 h-4 text-gray-500 mt-0.5 flex-shrink-0"
          aria-hidden="true"
        />
        <div className="flex-1 min-w-0">
          <div className="text-sm text-gray-700">{messageText}</div>
          {queueId && (
            <div className="text-xs text-gray-500 mt-1">
              Queue: {queueId}
            </div>
          )}
          {content && content.length > 0 && (
            <div className="text-xs text-gray-500 mt-1 truncate">
              {content}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
