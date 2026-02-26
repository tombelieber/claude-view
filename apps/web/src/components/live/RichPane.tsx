import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import Markdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeRaw from 'rehype-raw'
import {
  User,
  Bot,
  Brain,
  AlertTriangle,
  ArrowDown,
  Zap,
  BookOpen,
} from 'lucide-react'
import { ExpandProvider } from '../../contexts/ExpandContext'
import { CompactCodeBlock } from './CompactCodeBlock'
import { JsonTree } from './JsonTree'
import { isAskUserQuestionInput } from './AskUserQuestionDisplay'
import { useMonitorStore } from '../../store/monitor-store'
import type { ActionCategory } from './action-log/types'
import { ActionFilterChips } from './action-log/ActionFilterChips'
import { cn } from '../../lib/utils'
import {
  tryParseJson, isJsonContent, isDiffContent, isCodeLikeContent,
  stripLineNumbers, detectCodeLanguage,
} from '../../lib/content-detection'
import { markdownComponents } from '../../lib/markdown-components'
import { usePairedMessages, type DisplayItem } from '../../hooks/use-paired-messages'
import { PairedToolCard } from './PairedToolCard'
// System event cards (reused from MessageTyped)
import { TurnDurationCard } from '../TurnDurationCard'
import { ApiErrorCard } from '../ApiErrorCard'
import { CompactBoundaryCard } from '../CompactBoundaryCard'
import { HookSummaryCard } from '../HookSummaryCard'
import { LocalCommandEventCard } from '../LocalCommandEventCard'
import { MessageQueueEventCard } from '../MessageQueueEventCard'
import { FileSnapshotCard } from '../FileSnapshotCard'
import { SavedHookContextCard } from '../SavedHookContextCard'
import { SessionResultCard } from '../SessionResultCard'
// Progress event cards
import { AgentProgressCard } from '../AgentProgressCard'
import { BashProgressCard } from '../BashProgressCard'
import { HookProgressCard } from '../HookProgressCard'
import { HookEventRow } from './action-log/HookEventRow'
import { McpProgressCard } from '../McpProgressCard'
import { TaskQueueCard } from '../TaskQueueCard'
// Summary card
import { SessionSummaryCard } from '../SessionSummaryCard'

// --- Types ---

export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking' | 'error' | 'hook'
      | 'system' | 'progress' | 'summary'
  content: string
  name?: string // tool name for tool_use
  input?: string // tool input summary for tool_use
  inputData?: unknown // raw parsed object for tool_use (avoids re-parsing)
  ts?: number // timestamp
  category?: ActionCategory // set for tool_use, tool_result, hook, error
  metadata?: Record<string, unknown> // system/progress/summary subtype data
}

export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  /** When false (default), only show user + assistant + error messages. */
  verboseMode?: boolean
  /** Signals that the initial WebSocket buffer has been fully loaded.
   *  Triggers an imperative scroll-to-bottom so panes start at the latest message. */
  bufferDone?: boolean
  /** Pre-computed category counts from canonical message array. When provided,
   *  used directly for filter chips instead of computing internally. */
  categoryCounts?: Record<ActionCategory, number>
}

// --- Parser ---

/** Strip Claude Code internal command tags from content.
 * These tags appear in JSONL but are not meant for display:
 * <command-name>...</command-name>
 * <command-message>...</command-message>
 * <command-args>...</command-args>
 * <local-command-stdout>...</local-command-stdout>
 */
function stripCommandTags(content: string): string {
  return content
    .replace(/<command-name>[\s\S]*?<\/command-name>/g, '')
    .replace(/<command-message>[\s\S]*?<\/command-message>/g, '')
    .replace(/<command-args>[\s\S]*?<\/command-args>/g, '')
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, '')
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .trim()
}

/** Convert a timestamp value (ISO string or number) to Unix seconds, or undefined. */
function parseTimestamp(ts: unknown): number | undefined {
  if (typeof ts === 'number' && isFinite(ts) && ts > 0) return ts
  if (typeof ts === 'string') {
    const ms = Date.parse(ts)
    if (!isNaN(ms)) return ms / 1000
  }
  return undefined
}

/**
 * Parse a raw WebSocket/SSE message string into a structured RichMessage.
 * Returns null for messages that don't map to a displayable type.
 */
export function parseRichMessage(raw: string): RichMessage | null {
  try {
    const msg = JSON.parse(raw)
    if (msg.type === 'message') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content, null, 2))
      if (!content.trim()) return null
      return {
        type: msg.role === 'user' ? 'user' : 'assistant',
        content,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'tool_use') {
      return {
        type: 'tool_use',
        content: '',
        name: msg.name,
        input: msg.input ? JSON.stringify(msg.input, null, 2) : undefined,
        inputData: msg.input ?? undefined,
        ts: parseTimestamp(msg.ts),
        category: (msg.category as ActionCategory) ?? 'builtin',
      }
    }
    if (msg.type === 'tool_result') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content || '', null, 2))
      if (!content.trim()) return null
      return {
        type: 'tool_result',
        content,
        ts: parseTimestamp(msg.ts),
        category: (msg.category as ActionCategory) ?? undefined,
      }
    }
    if (msg.type === 'thinking') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : '')
      if (!content.trim()) return null
      return {
        type: 'thinking',
        content,
        ts: parseTimestamp(msg.ts),
      }
    }
    if (msg.type === 'error') {
      return {
        type: 'error',
        content: typeof msg.message === 'string' ? msg.message : JSON.stringify(msg, null, 2),
        category: 'error' as const,
      }
    }
    if (msg.type === 'line') {
      const content = stripCommandTags(typeof msg.data === 'string' ? msg.data : '')
      if (!content.trim()) return null
      return {
        type: 'assistant',
        content,
      }
    }
    if (msg.type === 'progress') {
      // Match messagesToRichMessages: hook_progress gets its own category
      const progressCategory = msg.metadata?.type === 'hook_progress'
        ? 'hook_progress' as ActionCategory
        : (msg.category as ActionCategory) ?? undefined
      return {
        type: 'progress',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
        metadata: msg.metadata,
        category: progressCategory,
      }
    }
    if (msg.type === 'system') {
      return {
        type: 'system',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
        category: (msg.category as ActionCategory) ?? 'system',
      }
    }
    if (msg.type === 'summary') {
      return {
        type: 'summary',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
        category: 'summary' as ActionCategory,
        metadata: msg.metadata,
      }
    }
    if (msg.type === 'result') {
      return {
        type: 'system',
        content: `Session result: ${msg.subtype || 'unknown'}`,
        ts: parseTimestamp(msg.ts),
        category: 'result' as ActionCategory,
        metadata: msg,
      }
    }
    return null
  } catch (e) {
    if (import.meta.env.DEV) console.warn('[parseRichMessage] failed to parse:', e)
    return null
  }
}

// --- Helpers ---

/** Format a timestamp as a static time label (chat-app style). Guards against epoch-zero. */
function formatTimestamp(ts: number | undefined): string | null {
  if (!ts || ts <= 0) return null
  const date = new Date(ts * 1000)
  if (isNaN(date.getTime())) return null
  const now = new Date()
  const time = date.toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' })
  // Today: "10:30 AM"
  if (date.toDateString() === now.toDateString()) return time
  // Yesterday: "Yesterday 10:30 AM"
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  if (date.toDateString() === yesterday.toDateString()) return `Yesterday ${time}`
  // This year: "Jan 15, 10:30 AM"
  if (date.getFullYear() === now.getFullYear()) {
    const month = date.toLocaleString(undefined, { month: 'short' })
    return `${month} ${date.getDate()}, ${time}`
  }
  // Older: "Jan 15 '25, 10:30 AM"
  const month = date.toLocaleString(undefined, { month: 'short' })
  return `${month} ${date.getDate()} '${String(date.getFullYear()).slice(-2)}, ${time}`
}

// --- Message Card Components ---

function UserMessage({ message, verboseMode = false }: { message: RichMessage; index?: number; verboseMode?: boolean }) {
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const jsonDetected = isJsonContent(message.content)
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null
  return (
    <div className="border-l-2 border-blue-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <User className="w-3 h-3 text-blue-500 dark:text-blue-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          {parsedJson !== null ? (
            verboseMode && richRenderMode === 'rich' ? (
              <JsonTree data={parsedJson} />
            ) : (
              <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`user-json-${message.ts ?? 0}`} />
            )
          ) : (
            <div className="text-xs text-gray-800 dark:text-gray-200 leading-relaxed prose dark:prose-invert prose-sm max-w-none">
              <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
            </div>
          )}
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function AssistantMessage({ message, verboseMode = false }: { message: RichMessage; index?: number; verboseMode?: boolean }) {
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const jsonDetected = isJsonContent(message.content)
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null
  return (
    <div className="pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <Bot className="w-3 h-3 text-gray-500 dark:text-gray-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          {parsedJson !== null ? (
            verboseMode && richRenderMode === 'rich' ? (
              <JsonTree data={parsedJson} />
            ) : (
              <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`asst-json-${message.ts ?? 0}`} />
            )
          ) : (
            <div className="text-xs text-gray-700 dark:text-gray-300 leading-relaxed prose dark:prose-invert prose-sm max-w-none">
              <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
            </div>
          )}
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function ToolResultMessage({ message, index, verboseMode = false }: { message: RichMessage; index: number; verboseMode?: boolean }) {
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const hasContent = message.content.length > 0
  const jsonDetected = hasContent && isJsonContent(message.content)
  const diffLike = hasContent && !jsonDetected && isDiffContent(message.content)
  const codeLike = hasContent && !jsonDetected && !diffLike && isCodeLikeContent(message.content)
  const codeLang = codeLike ? detectCodeLanguage(message.content) : 'text'
  // Strip line-number prefixes (e.g. "  42→ ") so Shiki can parse clean code
  const cleanCode = codeLike ? stripLineNumbers(message.content) : message.content

  // Always use JsonTree for JSON results — it collapses nested objects,
  // truncates long strings with tooltips, and avoids horizontal scroll.
  const parsedJson = jsonDetected ? tryParseJson(message.content) : null

  return (
    <div className="py-0.5 pl-3 border-l-2 border-gray-300/30 dark:border-gray-700/50 ml-1">
      <div className="flex items-center gap-1">
        <span className="text-[10px] text-gray-500 dark:text-gray-600 font-mono">result</span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {hasContent && (
        jsonDetected && parsedJson !== null ? (
          <div className="mt-0.5 pl-4">
            {richRenderMode === 'json' ? (
              <CompactCodeBlock code={JSON.stringify(parsedJson, null, 2)} language="json" blockId={`result-${index}`} />
            ) : (
              <JsonTree data={parsedJson} />
            )}
          </div>
        ) : diffLike ? (
          <div className="mt-0.5 pl-4 diff-block">
            <CompactCodeBlock code={message.content} language="diff" blockId={`result-${index}`} />
          </div>
        ) : codeLike ? (
          <div className="mt-0.5 pl-4">
            <CompactCodeBlock code={cleanCode} language={codeLang} blockId={`result-${index}`} />
          </div>
        ) : (
          <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 pl-4 font-mono leading-relaxed prose dark:prose-invert prose-sm max-w-none">
            <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
          </div>
        )
      )}
    </div>
  )
}

function ThinkingMessage({ message, verboseMode = false }: { message: RichMessage; verboseMode?: boolean }) {
  const [manualExpanded, setManualExpanded] = useState(false)
  const expanded = verboseMode || manualExpanded
  // Show a preview: first line or first ~120 chars
  const preview = useMemo(() => {
    const first = message.content.split('\n')[0] || ''
    return first.length > 120 ? first.slice(0, 120) + '…' : first
  }, [message.content])

  return (
    <div className="py-0.5">
      <button
        onClick={() => setManualExpanded((v) => !v)}
        className="flex items-center gap-1.5 w-full text-left cursor-pointer group"
      >
        <Brain className="w-3 h-3 text-purple-500/50 dark:text-purple-400/50 flex-shrink-0" />
        <span className="text-[10px] text-gray-500 dark:text-gray-600 italic">thinking...</span>
        <span className="text-[10px] text-gray-500 dark:text-gray-700 italic truncate flex-1 min-w-0 opacity-60 group-hover:opacity-100 transition-opacity">
          {preview}
        </span>
        <Timestamp ts={message.ts} />
      </button>
      {expanded && (
        <div className="text-[10px] text-gray-500 dark:text-gray-600 italic mt-0.5 pl-5 leading-relaxed prose dark:prose-invert prose-sm max-w-none border-l border-purple-500/20 ml-1.5">
          <Markdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeRaw]} components={markdownComponents}>{message.content}</Markdown>
        </div>
      )}
    </div>
  )
}

function ErrorMessage({ message, index }: { message: RichMessage; index: number }) {
  const jsonDetected = isJsonContent(message.content)
  return (
    <div className="border-l-2 border-red-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <AlertTriangle className="w-3 h-3 text-red-500 dark:text-red-400 flex-shrink-0 mt-0.5" />
        {jsonDetected ? (
          <div className="flex-1 min-w-0">
            <CompactCodeBlock code={message.content} language="json" blockId={`error-${index}`} />
          </div>
        ) : (
          <pre className="text-xs text-red-600 dark:text-red-300 whitespace-pre-wrap break-words font-sans leading-relaxed flex-1 min-w-0">
            {message.content}
          </pre>
        )}
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function HookMessage({ message }: { message: RichMessage }) {
  const [expanded, setExpanded] = useState(false)
  const hasContext = !!message.input
  return (
    <div className="border-l-2 border-amber-500/30 pl-2 py-0.5">
      <button
        onClick={() => hasContext && setExpanded((v) => !v)}
        className={cn(
          'flex items-center gap-1.5 w-full text-left',
          hasContext && 'cursor-pointer group',
        )}
      >
        <span className="w-1.5 h-1.5 rounded-full flex-shrink-0 bg-amber-400" />
        <span className="text-[10px] font-mono px-1 py-0.5 rounded bg-amber-500/10 text-amber-600 dark:text-amber-400 flex-shrink-0">
          {message.name || 'hook'}
        </span>
        <span className="text-[10px] text-gray-600 dark:text-gray-400 font-mono truncate flex-1 min-w-0">
          {message.content}
        </span>
        <Timestamp ts={message.ts} />
      </button>
      {expanded && message.input && (
        <pre className="text-[10px] font-mono text-amber-300/80 bg-gray-900 rounded p-2 mt-1 ml-5 overflow-x-auto max-h-[200px] overflow-y-auto whitespace-pre-wrap break-all">
          {(() => { try { return JSON.stringify(JSON.parse(message.input!), null, 2) } catch { return message.input } })()}
        </pre>
      )}
    </div>
  )
}

function Timestamp({ ts }: { ts?: number }) {
  const label = formatTimestamp(ts)
  if (!label) return null
  return (
    <span className="text-[9px] text-gray-400 dark:text-gray-600 tabular-nums flex-shrink-0 whitespace-nowrap">
      {label}
    </span>
  )
}

// --- System / Progress / Summary card dispatchers ---

function SystemMessageCard({ message, verboseMode }: { message: RichMessage; verboseMode?: boolean }) {
  const m = message.metadata
  const subtype = m?.type ?? m?.subtype
  const [cardOverride, setCardOverride] = useState<'rich' | 'json' | null>(null)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const effectiveMode = cardOverride ?? richRenderMode

  const card = (() => {
    switch (subtype) {
      case 'turn_duration':
        return <TurnDurationCard durationMs={m.durationMs} startTime={m.startTime} endTime={m.endTime} />
      case 'api_error':
        return <ApiErrorCard error={m.error} retryAttempt={m.retryAttempt} maxRetries={m.maxRetries} retryInMs={m.retryInMs} verboseMode={verboseMode} />
      case 'compact_boundary':
        return <CompactBoundaryCard trigger={m.trigger} preTokens={m.preTokens} postTokens={m.postTokens} />
      case 'hook_summary':
        return <HookSummaryCard hookCount={m.hookCount} hookInfos={m.hookInfos} hookErrors={m.hookErrors} durationMs={m.durationMs} preventedContinuation={m.preventedContinuation} verboseMode={verboseMode} />
      case 'local_command':
        return <LocalCommandEventCard content={m.content ?? message.content} />
      case 'queue-operation':
        return <MessageQueueEventCard operation={m.operation} timestamp={m.timestamp || ''} content={m.content} />
      case 'file-history-snapshot': {
        const snapshot = m.snapshot || {}
        const files = Object.keys(snapshot.trackedFileBackups || {})
        return <FileSnapshotCard fileCount={files.length} timestamp={snapshot.timestamp || ''} files={files} isIncremental={m.isSnapshotUpdate || false} verboseMode={verboseMode} />
      }
      case 'saved_hook_context': {
        const contentArr = Array.isArray(m.content) ? m.content : []
        return <SavedHookContextCard content={contentArr} verboseMode={verboseMode} />
      }
      case 'result':
        return <SessionResultCard subtype={m.subtype} durationMs={m.duration_ms} durationApiMs={m.duration_api_ms} numTurns={m.num_turns} isError={m.is_error} sessionId={m.session_id} />
      default:
        return null
    }
  })()

  return (
    <div className="border-l-2 border-amber-500/30 dark:border-amber-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <AlertTriangle className="w-3 h-3 text-amber-500/60 dark:text-amber-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-amber-600 dark:text-amber-400">system</span>
        {subtype && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{subtype}</span>
        )}
        {card !== null && (
          <button
            onClick={() => setCardOverride(effectiveMode === 'rich' ? 'json' : 'rich')}
            className={cn(
              'text-[10px] font-mono px-1 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
              effectiveMode === 'json'
                ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
                : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
            )}
            title={effectiveMode === 'rich' ? 'Switch to JSON view' : 'Switch to rich view'}
          >
            {'{ }'}
          </button>
        )}
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {card ? (
        effectiveMode === 'json' ? (
          <div className="mt-0.5 ml-5">
            <CompactCodeBlock code={JSON.stringify(m, null, 2)} language="json" />
          </div>
        ) : (
          <div className="mt-0.5 ml-5">{card}</div>
        )
      ) : message.content ? (
        <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 ml-5 font-mono">{message.content}</div>
      ) : m ? (
        <pre className="text-[10px] text-gray-500 dark:text-gray-600 mt-0.5 ml-5 font-mono whitespace-pre-wrap">{JSON.stringify(m, null, 2)}</pre>
      ) : null}
    </div>
  )
}

function ProgressMessageCard({ message, verboseMode }: { message: RichMessage; verboseMode?: boolean }) {
  const m = message.metadata
  const subtype = m?.type
  const [cardOverride, setCardOverride] = useState<'rich' | 'json' | null>(null)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const effectiveMode = cardOverride ?? richRenderMode

  const card = (() => {
    switch (subtype) {
      case 'agent_progress':
        return <AgentProgressCard agentId={m.agentId} prompt={m.prompt} model={m.model} tokens={m.tokens} normalizedMessages={m.normalizedMessages} indent={m.indent} verboseMode={verboseMode} />
      case 'bash_progress':
        return <BashProgressCard command={m.command} output={m.output} exitCode={m.exitCode} duration={m.duration} blockId={`bash-${message.ts ?? 0}`} />
      case 'hook_progress':
        return <HookProgressCard hookEvent={m.hookEvent} hookName={m.hookName} command={m.command} output={m.output} verboseMode={verboseMode} />
      case 'mcp_progress':
        return <McpProgressCard server={m.server} method={m.method} params={m.params} result={m.result} verboseMode={verboseMode} />
      case 'waiting_for_task':
        return <TaskQueueCard waitDuration={m.waitDuration} position={m.position} queueLength={m.queueLength} />
      case 'hook_event':
        return m._hookEvent
          ? <HookEventRow event={m._hookEvent} />
          : null
      default:
        return null
    }
  })()

  return (
    <div className="border-l-2 border-indigo-500/30 dark:border-indigo-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <Zap className="w-3 h-3 text-indigo-500/60 dark:text-indigo-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-indigo-600 dark:text-indigo-400">progress</span>
        {subtype && (
          <span className="text-[9px] font-mono text-gray-400 dark:text-gray-600">{subtype}</span>
        )}
        {card !== null && (
          <button
            onClick={() => setCardOverride(effectiveMode === 'rich' ? 'json' : 'rich')}
            className={cn(
              'text-[10px] font-mono px-1 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
              effectiveMode === 'json'
                ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
                : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
            )}
            title={effectiveMode === 'rich' ? 'Switch to JSON view' : 'Switch to rich view'}
          >
            {'{ }'}
          </button>
        )}
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {card ? (
        effectiveMode === 'json' ? (
          <div className="mt-0.5 ml-5">
            <CompactCodeBlock code={JSON.stringify(m, null, 2)} language="json" />
          </div>
        ) : (
          <div className="mt-0.5 ml-5">{card}</div>
        )
      ) : message.content ? (
        <div className="text-[10px] text-gray-600 dark:text-gray-500 mt-0.5 ml-5 font-mono">{message.content}</div>
      ) : m ? (
        <pre className="text-[10px] text-gray-500 dark:text-gray-600 mt-0.5 ml-5 font-mono whitespace-pre-wrap">{JSON.stringify(m, null, 2)}</pre>
      ) : null}
    </div>
  )
}

function SummaryMessageCard({ message, verboseMode }: { message: RichMessage; verboseMode?: boolean }) {
  const m = message.metadata
  const summary = m?.summary || message.content
  const leafUuid = m?.leafUuid || ''
  const wordCount = (summary || '').split(/\s+/).filter(Boolean).length
  const [cardOverride, setCardOverride] = useState<'rich' | 'json' | null>(null)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const effectiveMode = cardOverride ?? richRenderMode

  return (
    <div className="border-l-2 border-rose-500/30 dark:border-rose-500/20 pl-2 py-0.5">
      <div className="flex items-center gap-1.5">
        <BookOpen className="w-3 h-3 text-rose-500/60 dark:text-rose-400/50 flex-shrink-0" />
        <span className="text-[10px] font-mono text-rose-600 dark:text-rose-400">summary</span>
        <button
          onClick={() => setCardOverride(effectiveMode === 'rich' ? 'json' : 'rich')}
          className={cn(
            'text-[10px] font-mono px-1 py-0.5 rounded transition-colors duration-200 cursor-pointer flex-shrink-0',
            effectiveMode === 'json'
              ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 dark:bg-amber-500/20'
              : 'text-gray-400 dark:text-gray-600 hover:text-gray-600 dark:hover:text-gray-400',
          )}
          title={effectiveMode === 'rich' ? 'Switch to JSON view' : 'Switch to rich view'}
        >
          {'{ }'}
        </button>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {effectiveMode === 'json' ? (
        <div className="mt-0.5 ml-5">
          <CompactCodeBlock code={JSON.stringify(m || { summary, leafUuid, wordCount }, null, 2)} language="json" />
        </div>
      ) : (
        <div className="mt-0.5 ml-5">
          <SessionSummaryCard summary={summary} leafUuid={leafUuid} wordCount={wordCount} verboseMode={verboseMode} />
        </div>
      )}
    </div>
  )
}

// --- Message renderer dispatch ---

function MessageCard({ message, index, verboseMode = false }: { message: RichMessage; index: number; verboseMode?: boolean }) {
  switch (message.type) {
    case 'user':
      return <UserMessage message={message} index={index} verboseMode={verboseMode} />
    case 'assistant':
      return <AssistantMessage message={message} index={index} verboseMode={verboseMode} />
    case 'tool_result':
      return <ToolResultMessage message={message} index={index} verboseMode={verboseMode} />
    case 'thinking':
      return <ThinkingMessage message={message} verboseMode={verboseMode} />
    case 'error':
      return <ErrorMessage message={message} index={index} />
    case 'hook':
      return <HookMessage message={message} />
    case 'system':
      return <SystemMessageCard message={message} verboseMode={verboseMode} />
    case 'progress':
      return <ProgressMessageCard message={message} verboseMode={verboseMode} />
    case 'summary':
      return <SummaryMessageCard message={message} verboseMode={verboseMode} />
    default:
      return null
  }
}

function DisplayItemCard({ item, index, verboseMode = false }: { item: DisplayItem; index: number; verboseMode?: boolean }) {
  if (item.kind === 'tool_pair') {
    return <PairedToolCard toolUse={item.toolUse} toolResult={item.toolResult} index={index} verboseMode={verboseMode} />
  }
  // item.kind === 'message'
  return <MessageCard message={item.message} index={index} verboseMode={verboseMode} />
}

// --- Main Component ---

export function RichPane({ messages, isVisible, verboseMode = false, bufferDone = false, categoryCounts: countsProp }: RichPaneProps) {
  const verboseFilter = useMonitorStore((s) => s.verboseFilter)
  const setVerboseFilter = useMonitorStore((s) => s.setVerboseFilter)

  // Use prop if provided, otherwise compute internally (backward compat)
  const categoryCounts = useMemo(() => {
    if (countsProp) return countsProp
    const counts: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, hook_progress: 0, error: 0, system: 0, snapshot: 0, queue: 0, context: 0, result: 0, summary: 0 }
    if (!verboseMode) return counts
    for (const m of messages) {
      if (m.category) {
        counts[m.category] = (counts[m.category] || 0) + 1
      }
    }
    return counts
  }, [countsProp, messages, verboseMode])

  const displayMessages = useMemo(() => {
    if (!verboseMode) {
      return messages.filter((m) => {
        if (m.type === 'user' || m.type === 'error') return true
        if (m.type === 'assistant') {
          // Hide raw Task/sub-agent JSON blobs (e.g. {"task_id":...,"task_type":"local_agent"})
          const t = m.content.trim()
          if (t.startsWith('{') && t.includes('"task_id"') && t.includes('"task_type"')) return false
          return true
        }
        // Show AskUserQuestion in compact mode (friendly card, not raw JSON)
        if (m.type === 'tool_use' && m.name === 'AskUserQuestion' && isAskUserQuestionInput(m.inputData)) return true
        return false
      })
    }
    // Verbose mode: apply category filter
    if (verboseFilter === 'all') return messages
    return messages.filter((m) => {
      // Always show conversation backbone
      if (m.type === 'user' || m.type === 'assistant' || m.type === 'thinking') return true
      // Structural types: show if their category matches the filter
      if (m.type === 'system' || m.type === 'progress' || m.type === 'summary') {
        return !m.category || m.category === verboseFilter
      }
      // Filter by category
      return m.category === verboseFilter
    })
  }, [messages, verboseMode, verboseFilter])

  const displayItems = usePairedMessages(displayMessages)

  const renderItem = useCallback((index: number, item: DisplayItem) => (
    <div className="px-2 py-0.5">
      <DisplayItemCard item={item} index={index} verboseMode={verboseMode} />
    </div>
  ), [verboseMode])

  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [isAtBottom, setIsAtBottom] = useState(true)
  const [hasNewMessages, setHasNewMessages] = useState(false)
  const prevMessageCountRef = useRef(displayItems.length)
  const hasScrolledToBottomRef = useRef(false)
  const prevVerboseModeRef = useRef(verboseMode)

  // Jump to bottom once after initial buffer loads
  useEffect(() => {
    if (bufferDone && !hasScrolledToBottomRef.current && displayItems.length > 0) {
      hasScrolledToBottomRef.current = true
      // Use requestAnimationFrame to ensure Virtuoso has rendered the data
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({
          index: displayItems.length - 1,
          behavior: 'auto',
        })
      })
    }
  }, [bufferDone, displayItems.length])

  // Scroll to bottom when verbose mode toggles (list length changes drastically)
  useEffect(() => {
    if (prevVerboseModeRef.current !== verboseMode) {
      prevVerboseModeRef.current = verboseMode
      if (displayItems.length > 0) {
        requestAnimationFrame(() => {
          virtuosoRef.current?.scrollToIndex({
            index: displayItems.length - 1,
            behavior: 'auto',
          })
        })
      }
    }
  }, [verboseMode, displayItems.length])

  // Track when new messages arrive while user is scrolled up
  useEffect(() => {
    if (displayItems.length > prevMessageCountRef.current) {
      if (isAtBottom) {
        setHasNewMessages(false)
      } else {
        setHasNewMessages(true)
      }
    }
    prevMessageCountRef.current = displayItems.length
  }, [displayItems.length, isAtBottom])

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom)
    if (atBottom) {
      setHasNewMessages(false)
    }
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: displayItems.length - 1,
      behavior: 'smooth',
    })
    setHasNewMessages(false)
  }, [displayItems.length])

  if (!isVisible) return null

  if (displayItems.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-500 dark:text-gray-600">
        No messages yet
      </div>
    )
  }

  return (
    <ExpandProvider>
      <div className="relative h-full w-full flex flex-col">
        {verboseMode && (
          <ActionFilterChips
            counts={categoryCounts}
            activeFilter={verboseFilter}
            onFilterChange={setVerboseFilter as (filter: ActionCategory | 'all') => void}
          />
        )}
        <Virtuoso
          ref={virtuosoRef}
          data={displayItems}
          initialTopMostItemIndex={displayItems.length - 1}
          alignToBottom
          followOutput={'smooth'}
          atBottomStateChange={handleAtBottomStateChange}
          atBottomThreshold={30}
          itemContent={renderItem}
          className="h-full flex-1 min-h-0"
        />

        {/* "New messages" floating pill — click to scroll to latest */}
        {hasNewMessages && !isAtBottom && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-2 left-1/2 -translate-x-1/2 inline-flex items-center gap-1 bg-blue-600 hover:bg-blue-500 text-white text-xs px-3 py-1 rounded-full shadow-lg transition-colors z-10"
          >
            <ArrowDown className="w-3 h-3" />
            New messages
          </button>
        )}
      </div>
    </ExpandProvider>
  )
}
