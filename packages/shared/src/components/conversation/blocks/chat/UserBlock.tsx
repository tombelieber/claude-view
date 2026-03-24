import type { UserBlock as UserBlockType } from '../../../../types/blocks'
import { Bot, Check, GitBranch, Loader2, X } from 'lucide-react'
import { useConversationActions } from '../../../../contexts/conversation-actions-context'
import { MessageTimestamp } from '../shared/MessageTimestamp'

interface UserBlockProps {
  block: UserBlockType
}

function StatusDot({ status }: { status: UserBlockType['status'] }) {
  switch (status) {
    case 'optimistic':
      return (
        <span className="inline-block w-1.5 h-1.5 rounded-full bg-gray-400 dark:bg-gray-500 animate-pulse" />
      )
    case 'sending':
      return <Loader2 className="w-3 h-3 text-gray-400 dark:text-gray-500 animate-spin" />
    case 'sent':
      return <Check className="w-3 h-3 text-gray-400 dark:text-gray-500" />
    case 'failed':
      return <X className="w-3 h-3 text-red-500 dark:text-red-400" />
    default:
      return null
  }
}

export function ChatUserBlock({ block }: UserBlockProps) {
  const convActions = useConversationActions()
  const isSidechain = block.isSidechain === true

  return (
    <div
      data-testid="user-message"
      className={`flex justify-end ${isSidechain ? 'opacity-70 pl-6' : ''}`}
    >
      <div className="max-w-[80%]">
        {/* Agent / sidechain badges */}
        {(block.agentId || isSidechain) && (
          <div className="flex items-center justify-end gap-1.5 mb-1 px-1">
            {isSidechain && (
              <span className="inline-flex items-center gap-0.5 text-[10px] text-purple-500 dark:text-purple-400">
                <GitBranch className="w-2.5 h-2.5" />
                sidechain
              </span>
            )}
            {block.agentId && (
              <span className="inline-flex items-center gap-0.5 text-[10px] font-mono text-indigo-500 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/30 px-1.5 py-0.5 rounded-full">
                <Bot className="w-2.5 h-2.5" />
                {block.agentId}
              </span>
            )}
          </div>
        )}

        <div
          className={`px-3.5 py-2.5 rounded-2xl rounded-br-md text-white ${
            isSidechain
              ? 'bg-purple-400 dark:bg-purple-600 border border-purple-300 dark:border-purple-500 border-dashed'
              : block.agentId
                ? 'bg-indigo-500 dark:bg-indigo-600'
                : 'bg-blue-500 dark:bg-blue-600'
          }`}
        >
          <p className="text-sm whitespace-pre-wrap break-words">{block.text}</p>
        </div>

        {/* Image attachments */}
        {block.images && block.images.length > 0 && (
          <div className="mt-1.5 flex flex-wrap gap-1.5 justify-end">
            {block.images.map((img, i) => (
              <div
                key={`img-${i}`}
                className="rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700 max-w-[240px]"
              >
                {img.data ? (
                  <img
                    src={`data:${img.mediaType};base64,${img.data}`}
                    alt={`Attachment ${i + 1}`}
                    className="max-w-full h-auto"
                  />
                ) : img.url ? (
                  <img src={img.url} alt={`Attachment ${i + 1}`} className="max-w-full h-auto" />
                ) : (
                  <div className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400 bg-gray-100 dark:bg-gray-800">
                    Image ({img.mediaType})
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        <div className="flex items-center justify-end gap-1.5 mt-1 px-1">
          <MessageTimestamp timestamp={block.timestamp} align="right" />
          <StatusDot status={block.status} />
          {block.status === 'failed' && (
            <span className="text-[10px] text-red-500 dark:text-red-400">
              Failed
              {convActions && block.localId && (
                <>
                  {' · '}
                  <button
                    type="button"
                    onClick={() => convActions.retryMessage(block.localId!)}
                    className="underline hover:no-underline"
                  >
                    Retry
                  </button>
                </>
              )}
            </span>
          )}
        </div>
      </div>
    </div>
  )
}
