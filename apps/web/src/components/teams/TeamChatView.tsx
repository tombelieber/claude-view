import { useCallback, useEffect, useRef, useState } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import Markdown from 'react-markdown'
import { ChevronDown } from 'lucide-react'
import { markdownComponents } from '../../lib/markdown-components'
import { formatModelName } from '../../lib/format-model'
import { cn } from '../../lib/utils'
import type { InboxMessage, TeamMember } from '../../types/generated'

// ============================================================================
// Color maps
// ============================================================================

const BG_COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-50 dark:bg-blue-950/30',
  green: 'bg-green-50 dark:bg-green-950/30',
  yellow: 'bg-amber-50 dark:bg-amber-950/30',
  purple: 'bg-purple-50 dark:bg-purple-950/30',
  red: 'bg-red-50 dark:bg-red-950/30',
  orange: 'bg-orange-50 dark:bg-orange-950/30',
}

const BORDER_COLOR_MAP: Record<string, string> = {
  blue: 'border-blue-400 dark:border-blue-500',
  green: 'border-green-400 dark:border-green-500',
  yellow: 'border-amber-400 dark:border-amber-500',
  purple: 'border-purple-400 dark:border-purple-500',
  red: 'border-red-400 dark:border-red-500',
  orange: 'border-orange-400 dark:border-orange-500',
}

const DOT_COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-amber-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

const TEXT_COLOR_MAP: Record<string, string> = {
  blue: 'text-blue-700 dark:text-blue-300',
  green: 'text-green-700 dark:text-green-300',
  yellow: 'text-amber-700 dark:text-amber-300',
  purple: 'text-purple-700 dark:text-purple-300',
  red: 'text-red-700 dark:text-red-300',
  orange: 'text-orange-700 dark:text-orange-300',
}

// ============================================================================
// Helpers
// ============================================================================

function isProtocol(msg: InboxMessage): boolean {
  return (
    msg.messageType === 'idleNotification' ||
    msg.messageType === 'shutdownRequest' ||
    msg.messageType === 'shutdownApproved'
  )
}

/** Show time gap divider when messages are >2 min apart */
function shouldShowTimeDivider(prev: InboxMessage | undefined, curr: InboxMessage): boolean {
  if (!prev) return false
  const prevTs = new Date(prev.timestamp).getTime()
  const currTs = new Date(curr.timestamp).getTime()
  return currTs - prevTs > 2 * 60 * 1000
}

/** Collapse name/avatar when same speaker sends consecutive messages */
function shouldShowHeader(
  prev: InboxMessage | undefined,
  curr: InboxMessage,
  showDivider: boolean,
): boolean {
  if (!prev || showDivider) return true
  return prev.from !== curr.from
}

function formatTime(ts: string): string {
  try {
    return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  } catch {
    return ts.slice(11, 16)
  }
}

function getInitial(name: string): string {
  // "competition-champion" → "C", "team-lead" → "T"
  return (name[0] ?? '?').toUpperCase()
}

// ============================================================================
// Sub-components
// ============================================================================

function TimeDivider({ timestamp }: { timestamp: string }) {
  return (
    <div className="flex items-center gap-3 py-2 px-4">
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      <span className="text-[10px] font-medium text-gray-400 dark:text-gray-500 tabular-nums">
        {formatTime(timestamp)}
      </span>
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
    </div>
  )
}

function ChatBubble({
  msg,
  member,
  showHeader,
}: {
  msg: InboxMessage
  member: TeamMember | undefined
  showHeader: boolean
}) {
  const [expanded, setExpanded] = useState(false)
  const color = msg.color ?? member?.color ?? ''
  const isLead = member?.agentType === 'team-lead'
  const isLong = msg.text.length > 600

  // Moderator/team-lead messages: narrator style (no bubble)
  if (isLead) {
    return (
      <div className="px-4 py-1.5">
        {showHeader && (
          <div className="flex items-center gap-2 mb-1">
            <span className="text-xs font-medium text-gray-500 dark:text-gray-400">
              {member?.name ?? msg.from}
            </span>
            {member?.model && (
              <span className="text-[10px] text-gray-400 dark:text-gray-500">
                {formatModelName(member.model)}
              </span>
            )}
            <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums">
              {formatTime(msg.timestamp)}
            </span>
          </div>
        )}
        <div
          className={cn(
            'text-xs text-gray-600 dark:text-gray-400 prose prose-xs dark:prose-invert max-w-none',
            'bg-gray-50 dark:bg-gray-800/40 rounded-lg px-3 py-2',
            !expanded && isLong && 'line-clamp-4',
          )}
        >
          <Markdown components={markdownComponents}>{msg.text}</Markdown>
        </div>
        {isLong && (
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="text-[10px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 mt-0.5 ml-3"
          >
            {expanded ? 'collapse' : 'show more'}
          </button>
        )}
      </div>
    )
  }

  // Member messages: colored chat bubble
  const bgClass = BG_COLOR_MAP[color] ?? 'bg-gray-50 dark:bg-gray-800/40'
  const borderClass = BORDER_COLOR_MAP[color] ?? 'border-gray-300 dark:border-gray-600'
  const dotClass = DOT_COLOR_MAP[color] ?? 'bg-gray-400'
  const nameClass = TEXT_COLOR_MAP[color] ?? 'text-gray-700 dark:text-gray-300'

  return (
    <div className="px-4 py-0.5">
      {showHeader && (
        <div className="flex items-center gap-2 mb-1 ml-8">
          <span className={cn('text-xs font-semibold', nameClass)}>{member?.name ?? msg.from}</span>
          {member?.model && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
              {formatModelName(member.model)}
            </span>
          )}
          <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums">
            {formatTime(msg.timestamp)}
          </span>
        </div>
      )}
      <div className="flex items-start gap-2">
        {/* Avatar */}
        {showHeader ? (
          <div
            className={cn(
              'w-6 h-6 rounded-full flex items-center justify-center text-[10px] font-bold text-white shrink-0 mt-0.5',
              dotClass,
            )}
          >
            {getInitial(msg.from)}
          </div>
        ) : (
          <div className="w-6 shrink-0" /> /* spacer for alignment */
        )}
        {/* Bubble */}
        <div
          className={cn(
            'rounded-xl px-3 py-2 border-l-3 max-w-[85%]',
            bgClass,
            borderClass,
            !expanded && isLong && 'max-h-[200px] overflow-hidden relative',
          )}
        >
          <div className="text-xs text-gray-800 dark:text-gray-200 prose prose-xs dark:prose-invert max-w-none">
            <Markdown components={markdownComponents}>{msg.text}</Markdown>
          </div>
          {!expanded && isLong && (
            <div className="absolute bottom-0 left-0 right-0 h-12 bg-gradient-to-t from-white/90 dark:from-gray-950/90 to-transparent pointer-events-none rounded-b-xl" />
          )}
        </div>
      </div>
      {isLong && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="text-[10px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 mt-0.5 ml-8"
        >
          {expanded ? 'collapse' : `show more (${msg.text.length.toLocaleString()} chars)`}
        </button>
      )}
    </div>
  )
}

// ============================================================================
// Main component
// ============================================================================

interface TeamChatViewProps {
  messages: InboxMessage[]
  members: TeamMember[]
  topic?: string
}

export function TeamChatView({ messages, members, topic }: TeamChatViewProps) {
  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [atBottom, setAtBottom] = useState(true)
  const prevCountRef = useRef(messages.length)

  // Build member lookup
  const memberMap = new Map(members.map((m) => [m.name, m]))

  // Filter out protocol messages
  const visible = messages.filter((m) => !isProtocol(m))

  // Auto-scroll when new messages arrive (only if user was at bottom)
  useEffect(() => {
    if (visible.length > prevCountRef.current && atBottom) {
      // Small delay to let Virtuoso measure the new item
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({
          index: visible.length - 1,
          align: 'end',
          behavior: 'smooth',
        })
      })
    }
    prevCountRef.current = visible.length
  }, [visible.length, atBottom])

  const handleAtBottomChange = useCallback((bottom: boolean) => {
    setAtBottom(bottom)
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: visible.length - 1,
      align: 'end',
      behavior: 'smooth',
    })
  }, [visible.length])

  if (visible.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-sm text-gray-500 dark:text-gray-400">No messages yet</p>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Pinned header: topic + speakers */}
      {(topic || members.length > 0) && (
        <div className="flex-shrink-0 px-4 py-2.5 border-b border-gray-200 dark:border-gray-800 bg-gray-50/50 dark:bg-gray-900/50">
          {topic && (
            <h3 className="text-xs font-semibold text-gray-900 dark:text-gray-100 leading-snug mb-1.5">
              {topic}
            </h3>
          )}
          <div className="flex flex-wrap gap-2">
            {members
              .filter((m) => m.agentType !== 'team-lead')
              .map((m) => (
                <div key={m.agentId} className="flex items-center gap-1.5">
                  <span
                    className={cn(
                      'w-2 h-2 rounded-full shrink-0',
                      DOT_COLOR_MAP[m.color] ?? 'bg-gray-400',
                    )}
                  />
                  <span className="text-[11px] font-medium text-gray-700 dark:text-gray-300">
                    {m.name}
                  </span>
                  <span className="text-[10px] text-gray-400 dark:text-gray-500">
                    {formatModelName(m.model)}
                  </span>
                </div>
              ))}
          </div>
        </div>
      )}

      {/* Chat messages */}
      <div className="flex-1 min-h-0 relative">
        <Virtuoso
          ref={virtuosoRef}
          data={visible}
          alignToBottom
          followOutput="smooth"
          atBottomStateChange={handleAtBottomChange}
          atBottomThreshold={60}
          itemContent={(index, msg) => {
            const prev = index > 0 ? visible[index - 1] : undefined
            const showDivider = shouldShowTimeDivider(prev, msg)
            const showHeader = shouldShowHeader(prev, msg, showDivider)
            const member = memberMap.get(msg.from)

            return (
              <>
                {showDivider && <TimeDivider timestamp={msg.timestamp} />}
                <ChatBubble msg={msg} member={member} showHeader={showHeader} />
              </>
            )
          }}
        />

        {/* "New messages" button when scrolled up */}
        {!atBottom && (
          <button
            type="button"
            onClick={scrollToBottom}
            className="absolute bottom-3 left-1/2 -translate-x-1/2 flex items-center gap-1.5 px-3 py-1.5 rounded-full bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 text-xs font-medium shadow-lg hover:opacity-90 transition-opacity"
          >
            <ChevronDown className="w-3 h-3" />
            New messages
          </button>
        )}
      </div>
    </div>
  )
}
