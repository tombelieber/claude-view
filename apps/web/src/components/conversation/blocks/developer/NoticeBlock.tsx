import { CompactBoundaryCard } from '@claude-view/shared/components/CompactBoundaryCard'
import type { NoticeBlock as NoticeBlockType } from '@claude-view/shared/types/blocks'
import type {
  AssistantError,
  AuthStatus,
  ContextCompacted,
  ErrorEvent,
  PromptSuggestion,
  RateLimit,
  SessionClosed,
} from '@claude-view/shared/types/sidecar-protocol'
import { AlertCircle, Info, Loader2 } from 'lucide-react'

interface NoticeBlockProps {
  block: NoticeBlockType
  onSuggestion?: (text: string) => void
}

function AssistantErrorDetail({ data }: { data: AssistantError }) {
  return (
    <div className="flex items-start gap-2 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40">
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0 mt-0.5" />
      <div>
        <div className="text-xs font-medium text-red-700 dark:text-red-300">{data.error}</div>
        <div className="text-[10px] font-mono text-red-600 dark:text-red-400 mt-0.5">
          messageId: {data.messageId}
        </div>
      </div>
    </div>
  )
}

function RateLimitDetail({ data }: { data: RateLimit }) {
  const isWarning = data.status === 'allowed_warning'
  const isRejected = data.status === 'rejected'
  return (
    <div
      className={`flex items-start gap-2 px-3 py-2 rounded-lg border ${
        isRejected
          ? 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/40'
          : isWarning
            ? 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800/40'
            : 'bg-gray-50 dark:bg-gray-800/20 border-gray-200 dark:border-gray-700/40'
      }`}
    >
      <AlertCircle
        className={`w-4 h-4 flex-shrink-0 mt-0.5 ${
          isRejected
            ? 'text-red-500 dark:text-red-400'
            : isWarning
              ? 'text-yellow-500 dark:text-yellow-400'
              : 'text-gray-500 dark:text-gray-400'
        }`}
      />
      <div className="text-[11px] space-y-0.5">
        <div className="font-medium text-gray-700 dark:text-gray-300">
          Rate limit: {data.status}
        </div>
        {data.rateLimitType && (
          <div className="text-gray-500 dark:text-gray-400">Type: {data.rateLimitType}</div>
        )}
        {data.utilization != null && (
          <div className="text-gray-500 dark:text-gray-400">
            Utilization: {(data.utilization * 100).toFixed(0)}%
          </div>
        )}
        {data.resetsAt && data.resetsAt > 0 && (
          <div className="text-gray-500 dark:text-gray-400">
            Resets: {new Date(data.resetsAt * 1000).toLocaleTimeString()}
          </div>
        )}
      </div>
    </div>
  )
}

function ContextCompactedDetail({ data }: { data: ContextCompacted }) {
  return <CompactBoundaryCard trigger={data.trigger} preTokens={data.preTokens} />
}

function AuthStatusDetail({ data }: { data: AuthStatus }) {
  return (
    <div className="flex items-start gap-2 px-3 py-1.5 text-xs">
      {data.isAuthenticating ? (
        <Loader2 className="w-3.5 h-3.5 text-gray-400 animate-spin flex-shrink-0 mt-0.5" />
      ) : (
        <Info className="w-3.5 h-3.5 text-gray-400 flex-shrink-0 mt-0.5" />
      )}
      <div className="space-y-0.5">
        <div className="text-gray-600 dark:text-gray-300">
          {data.isAuthenticating ? 'Authenticating...' : 'Auth complete'}
        </div>
        {data.error && <div className="text-red-600 dark:text-red-400">{data.error}</div>}
        {data.output.length > 0 && (
          <pre className="text-[10px] font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap">
            {data.output.join('\n')}
          </pre>
        )}
      </div>
    </div>
  )
}

function SessionClosedDetail({ data }: { data: SessionClosed }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400">
      <Info className="w-3.5 h-3.5 flex-shrink-0" />
      <span>Session closed: {data.reason}</span>
    </div>
  )
}

function ErrorDetail({ data }: { data: ErrorEvent }) {
  return (
    <div
      className={`flex items-start gap-2 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40 ${
        data.fatal ? 'w-full' : ''
      }`}
    >
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0 mt-0.5" />
      <div>
        <div className="text-xs text-red-700 dark:text-red-300">{data.message}</div>
        <div className="text-[10px] text-red-500 dark:text-red-400 mt-0.5">
          {data.fatal ? 'Fatal' : 'Non-fatal'}
        </div>
      </div>
    </div>
  )
}

function PromptSuggestionDetail({
  data,
  onSuggestion,
}: {
  data: PromptSuggestion
  onSuggestion?: (text: string) => void
}) {
  return (
    <button
      type="button"
      onClick={() => onSuggestion?.(data.suggestion)}
      className="inline-flex items-center px-3 py-1.5 text-xs font-medium text-blue-700 dark:text-blue-300 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800/40 rounded-full hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors cursor-pointer"
    >
      {data.suggestion}
    </button>
  )
}

function SessionResumedDetail() {
  return (
    <div className="flex items-center gap-3 py-1">
      <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600 border-dashed" />
      <span className="text-[10px] text-gray-400 dark:text-gray-500">Resumed session</span>
      <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600 border-dashed" />
    </div>
  )
}

export function DevNoticeBlock({ block, onSuggestion }: NoticeBlockProps) {
  switch (block.variant) {
    case 'assistant_error':
      return <AssistantErrorDetail data={block.data as AssistantError} />
    case 'rate_limit':
      return <RateLimitDetail data={block.data as RateLimit} />
    case 'context_compacted':
      return <ContextCompactedDetail data={block.data as ContextCompacted} />
    case 'auth_status':
      return <AuthStatusDetail data={block.data as AuthStatus} />
    case 'session_closed':
      return <SessionClosedDetail data={block.data as SessionClosed} />
    case 'error':
      return <ErrorDetail data={block.data as ErrorEvent} />
    case 'prompt_suggestion':
      return (
        <PromptSuggestionDetail data={block.data as PromptSuggestion} onSuggestion={onSuggestion} />
      )
    case 'session_resumed':
      return <SessionResumedDetail />
    default:
      return null
  }
}
