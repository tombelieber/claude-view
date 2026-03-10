import { AlertCircle, Check, Loader2, RefreshCw, Wrench } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { ChatMessageWithStatus } from '../../types/control'
import { ThinkingBlock } from './ThinkingBlock'

interface LiveMessageBubbleProps {
  message: ChatMessageWithStatus
  onRetry?: (localId: string) => void
  verbose?: boolean
  toolResult?: { output: string; isError: boolean; duration?: number } | null
}

function StatusIndicator({ message, onRetry }: LiveMessageBubbleProps) {
  switch (message.status) {
    case 'optimistic':
      return (
        <span className="inline-block w-1.5 h-1.5 rounded-full bg-gray-400 dark:bg-gray-500 animate-pulse" />
      )
    case 'sending':
      return <Loader2 className="w-3 h-3 text-gray-400 dark:text-gray-500 animate-spin" />
    case 'sent':
      return <Check className="w-3 h-3 text-gray-400 dark:text-gray-500" />
    case 'failed':
      return (
        <span className="inline-flex items-center gap-1.5">
          <span className="text-xs text-red-500 dark:text-red-400">Failed to send</span>
          {onRetry && (
            <button
              type="button"
              onClick={() => onRetry(message.localId)}
              className="inline-flex items-center gap-1 text-xs text-blue-500 hover:text-blue-600 dark:text-blue-400 dark:hover:text-blue-300"
            >
              <RefreshCw className="w-3 h-3" />
              Retry
            </button>
          )}
        </span>
      )
    default:
      return null
  }
}

function formatTime(ts: number | undefined): string {
  if (!ts || ts <= 0) return ''
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

export function LiveMessageBubble({
  message,
  onRetry,
  verbose,
  toolResult,
}: LiveMessageBubbleProps) {
  // --- Thinking blocks ---
  if (message.role === 'thinking') {
    if (!verbose) return null // hidden in chat mode
    return (
      <div className="px-4 py-2">
        <ThinkingBlock content={message.content ?? ''} />
      </div>
    )
  }

  // --- Tool use ---
  if (message.role === 'tool_use') {
    if (!verbose) {
      // Compact: one-line summary
      return (
        <div className="flex items-center gap-2 px-4 py-2 text-xs text-gray-500 dark:text-gray-400">
          <Wrench className="w-3.5 h-3.5" />
          <span className="font-mono">{message.toolName ?? 'tool'}</span>
          {toolResult && (
            <span
              className={cn(
                'ml-auto',
                toolResult.isError
                  ? 'text-red-500 dark:text-red-400'
                  : 'text-green-500 dark:text-green-400',
              )}
            >
              {toolResult.isError ? 'error' : 'done'}
            </span>
          )}
        </div>
      )
    }
    // Verbose: show full tool input
    return (
      <div className="px-4 py-2 space-y-1">
        <div className="flex items-center gap-2 text-xs">
          <Wrench className="w-3.5 h-3.5 text-purple-500 dark:text-purple-400" />
          <span className="font-mono font-medium text-purple-700 dark:text-purple-300">
            {message.toolName ?? 'tool'}
          </span>
        </div>
        <pre className="text-[11px] text-gray-600 dark:text-gray-400 font-mono overflow-x-auto max-h-48 whitespace-pre-wrap bg-gray-50 dark:bg-gray-800/50 rounded p-2">
          {JSON.stringify(message.toolInput, null, 2)}
        </pre>
        {toolResult && (
          <div
            className={cn(
              'border-l-2 pl-2 mt-1',
              toolResult.isError
                ? 'border-red-300 dark:border-red-700'
                : 'border-green-300 dark:border-green-700',
            )}
          >
            <pre className="text-[11px] text-gray-600 dark:text-gray-400 font-mono overflow-x-auto max-h-48 whitespace-pre-wrap">
              {toolResult.output}
            </pre>
          </div>
        )}
      </div>
    )
  }

  // --- Tool result (standalone, only in verbose) ---
  if (message.role === 'tool_result') {
    if (!verbose) return null
    return (
      <div className="flex items-center gap-2 px-4 py-2 text-xs text-gray-500 dark:text-gray-400">
        {message.isError ? (
          <AlertCircle className="w-3.5 h-3.5 text-red-500 dark:text-red-400" />
        ) : (
          <Check className="w-3.5 h-3.5 text-green-500 dark:text-green-400" />
        )}
        <span className="font-mono truncate max-w-md">{message.output?.slice(0, 200)}</span>
      </div>
    )
  }

  // --- User / Assistant messages ---
  const isUser = message.role === 'user'
  const barColor = isUser ? 'bg-blue-500 dark:bg-blue-400' : 'bg-orange-500 dark:bg-orange-400'

  return (
    <div
      className={cn(
        'flex gap-3 px-4 py-3',
        message.status === 'failed' && 'border-l-2 border-red-500 dark:border-red-400',
      )}
    >
      <div className={cn('w-0.5 shrink-0 rounded-full', barColor)} />
      <div className="flex-1 min-w-0">
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words">
          {message.content}
        </p>
        <div className="flex items-center gap-2 mt-1.5">
          <span className="text-xs text-gray-400 dark:text-gray-500">
            {formatTime(message.createdAt)}
          </span>
          {isUser && <StatusIndicator message={message} onRetry={onRetry} />}
        </div>
      </div>
    </div>
  )
}
