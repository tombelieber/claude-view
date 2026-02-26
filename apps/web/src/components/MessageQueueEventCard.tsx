import { ListOrdered } from 'lucide-react'

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
      className="py-0.5 border-l-2 border-l-gray-400 pl-1 my-1"
      aria-label="Message queue event"
    >
      <div className="flex items-center gap-1.5">
        <ListOrdered className="w-3 h-3 text-gray-500 flex-shrink-0" aria-hidden="true" />
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400">
          {messageText}
        </span>
        {queueId && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-500">
            queue: {queueId}
          </span>
        )}
      </div>
      {content && content.length > 0 && (
        <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 ml-4 mt-0.5 truncate">
          {content}
        </div>
      )}
    </div>
  )
}
