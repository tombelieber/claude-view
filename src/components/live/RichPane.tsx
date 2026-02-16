import { useState, useRef, useEffect, useCallback, useMemo } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import Markdown from 'react-markdown'
import {
  ChevronDown,
  ChevronRight,
  User,
  Bot,
  Wrench,
  Brain,
  AlertTriangle,
  ArrowDown,
} from 'lucide-react'
import { cn } from '../../lib/utils'

// --- Types ---

export interface RichMessage {
  type: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'thinking' | 'error'
  content: string
  name?: string // tool name for tool_use
  input?: string // tool input summary for tool_use
  ts?: number // timestamp
}

export interface RichPaneProps {
  messages: RichMessage[]
  isVisible: boolean
  /** Whether to auto-follow new output. Disable during initial buffer load
   *  so the user sees content from the top, not the bottom. */
  followOutput?: boolean
  /** When false (default), only show user + assistant + error messages. */
  verboseMode?: boolean
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

/**
 * Parse a raw WebSocket/SSE message string into a structured RichMessage.
 * Returns null for messages that don't map to a displayable type.
 */
export function parseRichMessage(raw: string): RichMessage | null {
  try {
    const msg = JSON.parse(raw)
    if (msg.type === 'message') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content))
      if (!content.trim()) return null
      return {
        type: msg.role === 'user' ? 'user' : 'assistant',
        content,
        ts: msg.ts,
      }
    }
    if (msg.type === 'tool_use') {
      return {
        type: 'tool_use',
        content: '',
        name: msg.name,
        input: msg.input ? JSON.stringify(msg.input) : undefined,
        ts: msg.ts,
      }
    }
    if (msg.type === 'tool_result') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content || ''))
      if (!content.trim()) return null
      return {
        type: 'tool_result',
        content,
        ts: msg.ts,
      }
    }
    if (msg.type === 'thinking') {
      const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : '')
      if (!content.trim()) return null
      return {
        type: 'thinking',
        content,
        ts: msg.ts,
      }
    }
    if (msg.type === 'error') {
      return {
        type: 'error',
        content: typeof msg.message === 'string' ? msg.message : JSON.stringify(msg),
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
    return null
  } catch {
    return null
  }
}

// --- Helpers ---

/** Format a timestamp as relative time (e.g. "2s ago", "5m ago"). Guards against epoch-zero. */
function formatRelativeTime(ts: number | undefined): string | null {
  if (!ts || ts <= 0) return null
  const now = Date.now() / 1000
  const delta = Math.max(0, Math.floor(now - ts))
  if (delta < 5) return 'just now'
  if (delta < 60) return `${delta}s ago`
  if (delta < 3600) return `${Math.floor(delta / 60)}m ago`
  if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`
  return `${Math.floor(delta / 86400)}d ago`
}

// --- Message Card Components ---

function UserMessage({ message }: { message: RichMessage }) {
  return (
    <div className="border-l-2 border-blue-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <User className="w-3 h-3 text-blue-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          <div className="text-xs text-gray-200 leading-relaxed prose prose-invert prose-sm max-w-none">
            <Markdown>{message.content}</Markdown>
          </div>
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function AssistantMessage({ message }: { message: RichMessage }) {
  return (
    <div className="pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <Bot className="w-3 h-3 text-gray-400 flex-shrink-0 mt-0.5" />
        <div className="min-w-0 flex-1">
          <div className="text-xs text-gray-300 leading-relaxed prose prose-invert prose-sm max-w-none">
            <Markdown>{message.content}</Markdown>
          </div>
        </div>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function ToolUseMessage({ message }: { message: RichMessage }) {
  const label = message.name || 'Tool'
  const summary = message.input ? ` ${message.input}` : ''

  return (
    <div className="py-0.5">
      <div className="flex items-start gap-1.5">
        <Wrench className="w-3 h-3 text-orange-400 flex-shrink-0 mt-0.5" />
        <span className="inline-flex items-start gap-1 px-2 py-0.5 rounded bg-orange-500/20 text-orange-300 text-[10px] font-mono break-all">
          {label}
          {summary && (
            <span className="text-orange-400/70 break-all">{summary}</span>
          )}
        </span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function ToolResultMessage({ message }: { message: RichMessage }) {
  const hasContent = message.content.length > 0

  return (
    <div className="py-0.5 pl-5">
      <div className="flex items-center gap-1">
        <span className="text-[10px] text-gray-600 font-mono">result</span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      {hasContent && (
        <div className="text-[10px] text-gray-500 mt-0.5 pl-4 font-mono leading-relaxed prose prose-invert prose-sm max-w-none">
          <Markdown>{message.content}</Markdown>
        </div>
      )}
    </div>
  )
}

function ThinkingMessage({ message }: { message: RichMessage }) {
  return (
    <div className="py-0.5">
      <div className="flex items-center gap-1.5">
        <Brain className="w-3 h-3 text-purple-400/50 flex-shrink-0" />
        <span className="text-[10px] text-gray-600 italic">thinking...</span>
        <div className="flex-1" />
        <Timestamp ts={message.ts} />
      </div>
      <div className="text-[10px] text-gray-600 italic mt-0.5 pl-5 leading-relaxed prose prose-invert prose-sm max-w-none">
        <Markdown>{message.content}</Markdown>
      </div>
    </div>
  )
}

function ErrorMessage({ message }: { message: RichMessage }) {
  return (
    <div className="border-l-2 border-red-500 pl-2 py-1">
      <div className="flex items-start gap-1.5">
        <AlertTriangle className="w-3 h-3 text-red-400 flex-shrink-0 mt-0.5" />
        <pre className="text-xs text-red-300 whitespace-pre-wrap break-words font-sans leading-relaxed flex-1 min-w-0">
          {message.content}
        </pre>
        <Timestamp ts={message.ts} />
      </div>
    </div>
  )
}

function Timestamp({ ts }: { ts?: number }) {
  const relative = formatRelativeTime(ts)
  if (!relative) return null
  return (
    <span className="text-[9px] text-gray-600 tabular-nums flex-shrink-0 whitespace-nowrap">
      {relative}
    </span>
  )
}

// --- Message renderer dispatch ---

function MessageCard({ message }: { message: RichMessage }) {
  switch (message.type) {
    case 'user':
      return <UserMessage message={message} />
    case 'assistant':
      return <AssistantMessage message={message} />
    case 'tool_use':
      return <ToolUseMessage message={message} />
    case 'tool_result':
      return <ToolResultMessage message={message} />
    case 'thinking':
      return <ThinkingMessage message={message} />
    case 'error':
      return <ErrorMessage message={message} />
    default:
      return null
  }
}

// --- Main Component ---

export function RichPane({ messages, isVisible, followOutput: followOutputProp = true, verboseMode = false }: RichPaneProps) {
  const displayMessages = useMemo(() => {
    if (verboseMode) return messages
    return messages.filter((m) => m.type === 'user' || m.type === 'assistant' || m.type === 'error')
  }, [messages, verboseMode])

  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [isAtBottom, setIsAtBottom] = useState(true)
  const [hasNewMessages, setHasNewMessages] = useState(false)
  const prevMessageCountRef = useRef(displayMessages.length)

  // Track when new messages arrive while user is scrolled up
  useEffect(() => {
    if (displayMessages.length > prevMessageCountRef.current) {
      if (isAtBottom) {
        setHasNewMessages(false)
      } else {
        setHasNewMessages(true)
      }
    }
    prevMessageCountRef.current = displayMessages.length
  }, [displayMessages.length, isAtBottom])

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom)
    if (atBottom) {
      setHasNewMessages(false)
    }
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: displayMessages.length - 1,
      behavior: 'smooth',
    })
    setHasNewMessages(false)
  }, [displayMessages.length])

  if (!isVisible) return null

  if (displayMessages.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-xs text-gray-600">
        No messages yet
      </div>
    )
  }

  return (
    <div className="relative h-full w-full">
      <Virtuoso
        ref={virtuosoRef}
        data={displayMessages}
        followOutput={followOutputProp ? 'smooth' : false}
        atBottomStateChange={handleAtBottomStateChange}
        atBottomThreshold={30}
        itemContent={(_index, message) => (
          <div className="px-2 py-0.5">
            <MessageCard message={message} />
          </div>
        )}
        className="h-full"
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
  )
}
