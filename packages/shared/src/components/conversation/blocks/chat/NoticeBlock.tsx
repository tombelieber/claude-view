import type { NoticeBlock as NoticeBlockType } from '../../../../types/blocks'
import type {
  AssistantError,
  AuthStatus,
  ContextCompacted,
  ErrorEvent,
  PromptSuggestion,
  RateLimit,
  SessionClosed,
} from '../../../../types/sidecar-protocol'
import { AlertCircle, Info, Loader2 } from 'lucide-react'

interface NoticeBlockProps {
  block: NoticeBlockType
  onSuggestion?: (text: string) => void
}

function AssistantErrorNotice({ data }: { data: AssistantError }) {
  const labels: Record<string, string> = {
    rate_limit: 'Rate limited',
    billing_error: 'Billing error',
    authentication_failed: 'Authentication failed',
    invalid_request: 'Invalid request',
    server_error: 'Server error',
    max_output_tokens: 'Max output tokens reached',
    unknown: 'Unknown error',
  }
  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40">
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0" />
      <span className="text-xs text-red-700 dark:text-red-300">
        {labels[data.error] ?? data.error}
      </span>
    </div>
  )
}

function RateLimitNotice({
  data,
  retryInMs,
  retryAttempt,
  maxRetries,
}: {
  data: RateLimit
  retryInMs?: number
  retryAttempt?: number
  maxRetries?: number
}) {
  if (data.status === 'allowed') return null
  const isWarning = data.status === 'allowed_warning'
  const hasRetry = retryInMs != null && retryInMs > 0
  return (
    <div
      className={`flex items-center gap-2 px-3 py-2 rounded-lg border ${
        isWarning
          ? 'bg-yellow-50 dark:bg-yellow-900/20 border-yellow-200 dark:border-yellow-800/40'
          : 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800/40'
      }`}
    >
      <AlertCircle
        className={`w-4 h-4 flex-shrink-0 ${
          isWarning ? 'text-yellow-500 dark:text-yellow-400' : 'text-red-500 dark:text-red-400'
        }`}
      />
      <span
        className={`text-xs ${
          isWarning ? 'text-yellow-700 dark:text-yellow-300' : 'text-red-700 dark:text-red-300'
        }`}
      >
        {isWarning ? 'Approaching rate limit' : 'Rate limited'}
        {data.resetsAt && data.resetsAt > 0
          ? ` (resets ${new Date(data.resetsAt * 1000).toLocaleTimeString()})`
          : ''}
      </span>
      {hasRetry && (
        <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 ml-auto tabular-nums">
          retry {retryAttempt ?? '?'}/{maxRetries ?? '?'} in {(retryInMs / 1000).toFixed(0)}s
        </span>
      )}
    </div>
  )
}

function ContextCompactedNotice({ data }: { data: ContextCompacted }) {
  return (
    <div className="flex items-center gap-3 py-1">
      <div className="flex-1 h-px bg-gray-300/50 dark:bg-gray-600/50" />
      <span className="inline-flex items-center gap-1.5 text-[10px] text-gray-400 dark:text-gray-500">
        <Info className="w-3 h-3 flex-shrink-0" />
        Context compacted{data.trigger === 'manual' ? ' (manual)' : ''}
      </span>
      <div className="flex-1 h-px bg-gray-300/50 dark:bg-gray-600/50" />
    </div>
  )
}

function AuthStatusNotice({ data }: { data: AuthStatus }) {
  if (!data.isAuthenticating && !data.error) return null
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs">
      {data.isAuthenticating ? (
        <>
          <Loader2 className="w-3.5 h-3.5 text-gray-400 animate-spin" />
          <span className="text-gray-500 dark:text-gray-400">Authenticating...</span>
        </>
      ) : (
        <>
          <AlertCircle className="w-3.5 h-3.5 text-red-500 dark:text-red-400" />
          <span className="text-red-600 dark:text-red-400">{data.error}</span>
        </>
      )}
    </div>
  )
}

function SessionClosedNotice({ data }: { data: SessionClosed }) {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 text-xs text-gray-500 dark:text-gray-400">
      <Info className="w-3.5 h-3.5 flex-shrink-0" />
      <span>Session ended: {data.reason}</span>
    </div>
  )
}

function extractErrorMessage(data: ErrorEvent): string {
  // data.message can be a string (normal) or a raw API response object
  // (from history JSONL where isApiErrorMessage=true)
  if (typeof data.message === 'string') return data.message
  if (data.message && typeof data.message === 'object') {
    const msg = data.message as Record<string, unknown>
    // Extract text from API response content array
    if (Array.isArray(msg.content)) {
      const textBlock = msg.content.find(
        (c: unknown) =>
          typeof c === 'object' && c !== null && (c as Record<string, unknown>).type === 'text',
      ) as { text?: string } | undefined
      if (textBlock?.text) return textBlock.text
    }
  }
  // Fallback: use the error field from the parent data or stringify
  const raw = data as unknown as Record<string, unknown>
  if (typeof raw.error === 'string') return raw.error
  return 'Unknown error'
}

function ErrorNotice({ data }: { data: ErrorEvent }) {
  return (
    <div
      className={`flex items-center gap-2 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40 ${
        data.fatal ? 'w-full' : ''
      }`}
    >
      <AlertCircle className="w-4 h-4 text-red-500 dark:text-red-400 flex-shrink-0" />
      <span className="text-xs text-red-700 dark:text-red-300">{extractErrorMessage(data)}</span>
    </div>
  )
}

function PromptSuggestionNotice({
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

function SessionResumedNotice() {
  return (
    <div className="flex items-center gap-3 py-1">
      <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600 border-dashed" />
      <span className="text-[10px] text-gray-400 dark:text-gray-500">Resumed session</span>
      <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600 border-dashed" />
    </div>
  )
}

export function ChatNoticeBlock({ block, onSuggestion }: NoticeBlockProps) {
  switch (block.variant) {
    case 'assistant_error':
      return <AssistantErrorNotice data={block.data as AssistantError} />
    case 'rate_limit':
      return (
        <RateLimitNotice
          data={block.data as RateLimit}
          retryInMs={block.retryInMs}
          retryAttempt={block.retryAttempt}
          maxRetries={block.maxRetries}
        />
      )
    case 'context_compacted':
      return <ContextCompactedNotice data={block.data as ContextCompacted} />
    case 'auth_status':
      return <AuthStatusNotice data={block.data as AuthStatus} />
    case 'session_closed':
      return <SessionClosedNotice data={block.data as SessionClosed} />
    case 'error':
      return <ErrorNotice data={block.data as ErrorEvent} />
    case 'prompt_suggestion':
      return (
        <PromptSuggestionNotice data={block.data as PromptSuggestion} onSuggestion={onSuggestion} />
      )
    case 'session_resumed':
      return <SessionResumedNotice />
    default:
      return null
  }
}
