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
import { CompactBoundaryCard } from '../../../CompactBoundaryCard'
import { EventCard } from './EventCard'

interface NoticeBlockProps {
  block: NoticeBlockType
  onSuggestion?: (text: string) => void
}

function AssistantErrorDetail({ data }: { data: AssistantError }) {
  return (
    <EventCard dot="red" chip="Error" label={data.error} error rawData={data}>
      <span className="text-xs font-mono text-gray-500 dark:text-gray-400">
        messageId: {data.messageId}
      </span>
    </EventCard>
  )
}

function RateLimitDetail({ data }: { data: RateLimit }) {
  const isRejected = data.status === 'rejected'
  const isWarning = data.status === 'allowed_warning'
  const dot = isRejected ? ('red' as const) : isWarning ? ('amber' as const) : ('gray' as const)

  return (
    <EventCard
      dot={dot}
      chip="Rate Limit"
      chipColor={isRejected ? undefined : undefined}
      label={data.status}
      error={isRejected}
      rawData={data}
    >
      <div className="text-xs space-y-0.5 text-gray-500 dark:text-gray-400">
        {data.rateLimitType && <div>Type: {data.rateLimitType}</div>}
        {data.utilization != null && <div>Utilization: {(data.utilization * 100).toFixed(0)}%</div>}
        {data.resetsAt && data.resetsAt > 0 && (
          <div>Resets: {new Date(data.resetsAt * 1000).toLocaleTimeString()}</div>
        )}
      </div>
    </EventCard>
  )
}

function ContextCompactedDetail({ data }: { data: ContextCompacted }) {
  return (
    <EventCard
      dot="cyan"
      chip="Compacted"
      label={`${data.trigger} — ${(data.preTokens ?? 0).toLocaleString()} tokens`}
      rawData={data}
    >
      <CompactBoundaryCard trigger={data.trigger} preTokens={data.preTokens} />
    </EventCard>
  )
}

function AuthStatusDetail({ data }: { data: AuthStatus }) {
  return (
    <EventCard
      dot={data.error ? 'red' : data.isAuthenticating ? 'amber' : 'green'}
      chip="Auth"
      label={data.isAuthenticating ? 'Authenticating...' : data.error ? data.error : 'Complete'}
      pulse={data.isAuthenticating}
      error={!!data.error}
      rawData={data}
    >
      {data.output.length > 0 && (
        <pre className="text-xs font-mono text-gray-500 dark:text-gray-400 whitespace-pre-wrap">
          {data.output.join('\n')}
        </pre>
      )}
    </EventCard>
  )
}

function SessionClosedDetail({ data }: { data: SessionClosed }) {
  return <EventCard dot="gray" chip="Closed" label={data.reason} rawData={data} />
}

function ErrorDetail({ data }: { data: ErrorEvent }) {
  return (
    <EventCard
      dot="red"
      chip={data.fatal ? 'Fatal' : 'Error'}
      label={data.message}
      error
      rawData={data}
    />
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
    <EventCard dot="blue" chip="Suggestion" label={data.suggestion.slice(0, 60)}>
      <button
        type="button"
        onClick={() => onSuggestion?.(data.suggestion)}
        className="inline-flex items-center px-3 py-1.5 text-xs font-medium text-blue-700 dark:text-blue-300 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800/40 rounded-full hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors duration-200 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50"
      >
        {data.suggestion}
      </button>
    </EventCard>
  )
}

function SessionResumedDetail() {
  return (
    <EventCard dot="gray" chip="Session" label="Resumed">
      <div className="flex items-center gap-3 py-0.5">
        <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600" />
        <span className="text-xs text-gray-500 dark:text-gray-400">Resumed session</span>
        <div className="flex-1 h-px bg-gray-300 dark:bg-gray-600" />
      </div>
    </EventCard>
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
