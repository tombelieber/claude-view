import type { SessionPhase } from '../../hooks/use-session-control'
import { cn } from '../../lib/utils'

interface StreamingFooterProps {
  streamingContent: string
  phase: SessionPhase
  liveMessageCount: number
  totalMessages: number
  hiddenCount: number
}

export function StreamingFooter({
  streamingContent,
  phase,
  liveMessageCount,
  totalMessages,
  hiddenCount,
}: StreamingFooterProps) {
  if (phase === 'connecting' || phase === 'reconnecting') {
    return (
      <div className="flex items-center gap-2 px-4 py-3 text-sm text-gray-500 dark:text-gray-400">
        <span className="inline-block w-2 h-2 rounded-full bg-blue-400 dark:bg-blue-500 animate-pulse" />
        <span>Resuming session...</span>
      </div>
    )
  }

  if (streamingContent) {
    return (
      <div className="flex gap-3 px-4 py-3">
        <div className="w-0.5 shrink-0 rounded-full bg-orange-500/60 dark:bg-orange-400/60" />
        <div className="flex-1 min-w-0">
          <p
            className={cn(
              'text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap break-words',
              'bg-orange-50/50 dark:bg-orange-950/20 -mx-2 px-2 py-1 rounded',
            )}
          >
            {streamingContent}
            <span className="inline-block w-[2px] h-4 ml-0.5 align-text-bottom bg-orange-500 dark:bg-orange-400 animate-[blink_1s_step-end_infinite]" />
          </p>
        </div>
      </div>
    )
  }

  if (liveMessageCount === 0 && totalMessages > 0) {
    return (
      <div className="px-4 py-2 text-center text-xs text-gray-400 dark:text-gray-500">
        {totalMessages} message{totalMessages !== 1 ? 's' : ''}
        {hiddenCount > 0 && ` (${hiddenCount} hidden)`}
      </div>
    )
  }

  return null
}
