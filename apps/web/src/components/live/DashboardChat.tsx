import { useCallback, useEffect, useRef, useState } from 'react'
import { useControlSession } from '../../hooks/use-control-session'
import { useSessionMessages } from '../../hooks/use-session-messages'
import type { ChatMessage } from '../../types/control'
import type { Message } from '../../types/generated'
import { ChatStatusBar } from './ChatStatusBar'
import { PermissionDialog } from './PermissionDialog'

interface DashboardChatProps {
  controlId: string
  sessionId: string
}

export function DashboardChat({ controlId, sessionId }: DashboardChatProps) {
  const session = useControlSession(controlId)
  const historyQuery = useSessionMessages(sessionId)
  const [input, setInput] = useState('')
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const [autoScroll, setAutoScroll] = useState(true)

  // Flatten paginated history
  const history = historyQuery.data?.pages.flatMap((p) => p.messages) ?? []

  // Auto-scroll to bottom when new messages arrive or streaming content changes.
  // messageCount and streamingLen are intentional trigger deps — they aren't
  // referenced in the body but the effect must re-run when they change.
  const messageCount = session.messages.length
  const streamingLen = session.streamingContent.length
  // biome-ignore lint/correctness/useExhaustiveDependencies: messageCount and streamingLen are intentional trigger deps for scroll
  useEffect(() => {
    if (autoScroll && messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messageCount, streamingLen, autoScroll])

  const handleScroll = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    const isAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 100
    setAutoScroll(isAtBottom)
  }, [])

  const handleSend = useCallback(() => {
    const trimmed = input.trim()
    if (!trimmed) return
    session.sendMessage(trimmed)
    setInput('')
  }, [input, session.sendMessage])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      }
    },
    [handleSend],
  )

  const isInputDisabled =
    session.status === 'completed' ||
    session.status === 'error' ||
    session.status === 'disconnected'

  return (
    <div className="flex flex-col h-full bg-white dark:bg-gray-950">
      {/* Status bar */}
      <ChatStatusBar
        contextUsage={session.contextUsage}
        turnCount={session.turnCount}
        sessionCost={session.sessionCost}
        lastTurnCost={session.lastTurnCost}
        status={session.status}
      />

      {/* Messages */}
      <div
        ref={containerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto px-4 py-4 space-y-3"
      >
        {/* Historical messages */}
        {historyQuery.isLoading && (
          <div className="text-sm text-gray-400 dark:text-gray-500 text-center py-4">
            Loading history...
          </div>
        )}
        {history.map((msg, i) => (
          <HistoryMessage key={`hist-${msg.uuid ?? i}`} message={msg} />
        ))}

        {/* Divider between history and live session */}
        {history.length > 0 && (
          <div className="flex items-center gap-3 py-3">
            <div className="flex-1 h-px bg-blue-200 dark:bg-blue-800" />
            <span className="text-xs font-medium text-blue-500 dark:text-blue-400 whitespace-nowrap">
              Dashboard session started
            </span>
            <div className="flex-1 h-px bg-blue-200 dark:bg-blue-800" />
          </div>
        )}

        {/* New control session messages */}
        {session.messages.map((msg, i) => (
          <ControlMessage key={`ctrl-${msg.messageId ?? msg.toolUseId ?? i}`} message={msg} />
        ))}

        {/* Streaming content */}
        {session.streamingContent && (
          <div className="rounded-lg bg-gray-50 dark:bg-gray-900 p-3">
            <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap">
              {session.streamingContent}
              <span className="inline-block w-2 h-4 bg-blue-500 animate-pulse ml-0.5" />
            </p>
          </div>
        )}

        {/* Permission request dialog */}
        <PermissionDialog
          request={session.permissionRequest}
          onRespond={session.respondPermission}
        />

        {/* Session completed banner */}
        {session.status === 'completed' && (
          <div className="rounded-lg bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 p-3 text-center">
            <p className="text-sm font-medium text-green-700 dark:text-green-400">
              Session completed
            </p>
            <p className="text-xs text-green-600 dark:text-green-500 mt-1">
              Total cost: ${session.sessionCost.toFixed(4)} | {session.turnCount} turns
            </p>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="border-t border-gray-200 dark:border-gray-800 p-4">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isInputDisabled}
            placeholder={
              isInputDisabled
                ? 'Session ended'
                : 'Send a message... (Enter to send, Shift+Enter for newline)'
            }
            rows={1}
            className="flex-1 resize-none rounded-lg border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-900 px-3 py-2 text-sm text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50"
          />
          <button
            type="button"
            onClick={handleSend}
            disabled={isInputDisabled || !input.trim()}
            className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  )
}

/** Render a historical message from the session's JSONL history */
function HistoryMessage({ message }: { message: Message }) {
  const isUser = message.role === 'user'
  return (
    <div
      className={`rounded-lg p-3 ${isUser ? 'bg-blue-50 dark:bg-blue-900/20 ml-8' : 'bg-gray-50 dark:bg-gray-900 mr-8'}`}
    >
      <div className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">
        {isUser ? 'You' : 'Claude'}
      </div>
      <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap">
        {message.content}
      </p>
    </div>
  )
}

/** Render a control session message */
function ControlMessage({ message }: { message: ChatMessage }) {
  if (message.role === 'user') {
    return (
      <div className="rounded-lg p-3 bg-blue-50 dark:bg-blue-900/20 ml-8">
        <div className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">You</div>
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap">
          {message.content}
        </p>
      </div>
    )
  }

  if (message.role === 'assistant') {
    return (
      <div className="rounded-lg p-3 bg-gray-50 dark:bg-gray-900 mr-8">
        <div className="text-xs font-medium text-gray-500 dark:text-gray-400 mb-1">Claude</div>
        <p className="text-sm text-gray-900 dark:text-gray-100 whitespace-pre-wrap">
          {message.content}
        </p>
      </div>
    )
  }

  if (message.role === 'tool_use') {
    return <ToolCallBlock toolName={message.toolName ?? 'unknown'} toolInput={message.toolInput} />
  }

  if (message.role === 'tool_result') {
    return (
      <div className="rounded-lg border border-gray-200 dark:border-gray-700 p-2 mx-4">
        <div className="text-xs text-gray-500 dark:text-gray-400">Tool Result</div>
        <pre
          className={`text-xs mt-1 whitespace-pre-wrap ${message.isError ? 'text-red-600 dark:text-red-400' : 'text-gray-700 dark:text-gray-300'}`}
        >
          {message.output}
        </pre>
      </div>
    )
  }

  return null
}

/** Collapsible tool call block */
function ToolCallBlock({
  toolName,
  toolInput,
}: { toolName: string; toolInput?: Record<string, unknown> }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 mx-4 overflow-hidden">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs font-medium text-gray-600 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800"
      >
        <span className={`transition-transform ${expanded ? 'rotate-90' : ''}`}>&#9654;</span>
        <span className="font-mono">{toolName}</span>
        {toolName === 'Bash' && toolInput?.command != null && (
          <span className="font-mono text-gray-400 dark:text-gray-500 truncate">
            {String(toolInput.command).slice(0, 60)}
          </span>
        )}
      </button>
      {expanded && toolInput && (
        <pre className="px-3 py-2 text-xs text-gray-700 dark:text-gray-300 bg-gray-50 dark:bg-gray-800/50 border-t border-gray-200 dark:border-gray-700 overflow-x-auto whitespace-pre-wrap">
          {JSON.stringify(toolInput, null, 2)}
        </pre>
      )}
    </div>
  )
}
