import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import Markdown from 'react-markdown'
import { CheckCircle2, ChevronDown, Crown } from 'lucide-react'
import { markdownComponents } from '../../lib/markdown-components'
import { formatModelName } from '../../lib/format-model'
import { formatCostUsd } from '../../lib/format-utils'
import { cn } from '../../lib/utils'
import type { InboxMessage, TeamMember } from '../../types/generated'
import type { TeamMemberSidechain } from '@claude-view/shared/types/generated/TeamMemberSidechain'
import { StructuredMessageCard } from './StructuredMessageCard'

// ============================================================================
// Timeline types — union of messages and sidechain completion events
// ============================================================================

type TimelineItem =
  | { kind: 'message'; msg: InboxMessage }
  | { kind: 'work-done'; sidechain: TeamMemberSidechain; timestamp: string }

/** Merge inbox messages and per-sidechain completion events into a single sorted timeline. */
function buildTimeline(
  messages: InboxMessage[],
  sidechains: TeamMemberSidechain[] | undefined,
): TimelineItem[] {
  const items: TimelineItem[] = messages
    .filter((m) => !isProtocol(m))
    .map((msg) => ({ kind: 'message' as const, msg }))

  if (sidechains && sidechains.length > 0) {
    for (const sc of sidechains) {
      if (sc.endedAt) {
        items.push({ kind: 'work-done', sidechain: sc, timestamp: sc.endedAt })
      }
    }
  }

  items.sort((a, b) => {
    const tsA = a.kind === 'message' ? a.msg.timestamp : a.timestamp
    const tsB = b.kind === 'message' ? b.msg.timestamp : b.timestamp
    return tsA.localeCompare(tsB)
  })

  return items
}

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

/** Try parsing msg.text as structured JSON. Returns parsed data or null. */
function tryParseStructured(text: string): Record<string, unknown> | null {
  if (!text.startsWith('{')) return null
  try {
    const data = JSON.parse(text)
    // Must be an object with a `type` field to qualify as structured
    if (data && typeof data === 'object' && typeof data.type === 'string') return data
    return null
  } catch {
    return null
  }
}

// ============================================================================
// Sub-components
// ============================================================================

/** Format seconds → compact duration (e.g. "3m 12s", "21m"). */
function fmtDuration(s: number): string {
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  const rem = s % 60
  return rem > 0 ? `${m}m ${rem}s` : `${m}m`
}

/** Inline badge: a single sidechain session completed. Clickable → opens JSONL drill-down. */
function WorkDoneBadge({
  sidechain,
  member,
  onSelect,
}: {
  sidechain: TeamMemberSidechain
  member: TeamMember | undefined
  onSelect?: (target: { hexId: string; memberName: string }) => void
}) {
  const color = member?.color ?? ''
  const dotClass = DOT_COLOR_MAP[color] ?? 'bg-gray-400'
  const clickable = !!onSelect

  return (
    <div className="flex items-center justify-center py-2 px-5">
      <button
        type="button"
        disabled={!clickable}
        onClick={() => onSelect?.({ hexId: sidechain.hexId, memberName: sidechain.memberName })}
        className={cn(
          'flex items-center gap-2 px-3 py-1.5 rounded-full bg-green-50 dark:bg-green-950/30 border border-green-200 dark:border-green-800 text-xs text-gray-500 dark:text-gray-400 transition-colors',
          clickable &&
            'cursor-pointer hover:bg-green-100 dark:hover:bg-green-900/40 hover:border-green-300 dark:hover:border-green-700',
        )}
      >
        <CheckCircle2 className="w-3.5 h-3.5 text-green-500 dark:text-green-400 shrink-0" />
        <span className={cn('w-2 h-2 rounded-full shrink-0', dotClass)} />
        <span className="font-medium text-gray-700 dark:text-gray-300">{sidechain.memberName}</span>
        <span>completed</span>
        <span className="text-gray-300 dark:text-gray-600">·</span>
        <span className="tabular-nums">{fmtDuration(sidechain.durationSeconds)}</span>
        {sidechain.costUsd != null && sidechain.costUsd > 0 && (
          <>
            <span className="text-gray-300 dark:text-gray-600">·</span>
            <span className="tabular-nums font-mono">{formatCostUsd(sidechain.costUsd)}</span>
          </>
        )}
        {clickable && (
          <ChevronDown className="w-3 h-3 -rotate-90 text-gray-400 dark:text-gray-500 shrink-0" />
        )}
      </button>
    </div>
  )
}

function TimeDivider({ timestamp }: { timestamp: string }) {
  return (
    <div className="flex items-center gap-4 py-3 px-5">
      <div className="flex-1 h-px bg-gray-200 dark:bg-gray-700" />
      <span className="text-xs font-medium text-gray-400 dark:text-gray-500 tabular-nums">
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
  const structured = tryParseStructured(msg.text)
  const isLong = msg.text.length > 600

  // Avatar element — reused across all message types
  const dotClass = DOT_COLOR_MAP[color] ?? 'bg-gray-400'
  const avatar = showHeader ? (
    isLead ? (
      <div className="w-8 h-8 rounded-full flex items-center justify-center bg-amber-100 dark:bg-amber-900/40 shrink-0">
        <Crown className="w-4 h-4 text-amber-500 dark:text-amber-400" />
      </div>
    ) : (
      <div
        className={cn(
          'w-8 h-8 rounded-full flex items-center justify-center text-xs font-bold text-white shrink-0',
          dotClass,
        )}
      >
        {getInitial(msg.from)}
      </div>
    )
  ) : (
    <div className="w-8 shrink-0" /> /* spacer for alignment */
  )

  // Name + model + time header
  const nameClass = isLead
    ? 'text-amber-700 dark:text-amber-300'
    : (TEXT_COLOR_MAP[color] ?? 'text-gray-700 dark:text-gray-300')
  const header = showHeader && (
    <div className="flex items-center gap-2 mb-1">
      {isLead && <Crown className="w-3.5 h-3.5 shrink-0 text-amber-500 dark:text-amber-400" />}
      <span className={cn('text-sm font-semibold', nameClass)}>{member?.name ?? msg.from}</span>
      {member?.model && (
        <span className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
          {formatModelName(member.model)}
        </span>
      )}
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
        {formatTime(msg.timestamp)}
      </span>
    </div>
  )

  // Structured JSON messages → EventCard-style card
  if (structured) {
    return (
      <div className={cn('px-5', showHeader ? 'pt-5 pb-1' : 'py-1')}>
        <div className="flex items-start gap-3">
          {avatar}
          <div className="flex-1 min-w-0">
            {header}
            <StructuredMessageCard data={structured} rawText={msg.text} />
          </div>
        </div>
      </div>
    )
  }

  // All messages (lead + members): same avatar + bubble layout
  const bgClass = isLead
    ? 'bg-amber-50/60 dark:bg-amber-950/20'
    : (BG_COLOR_MAP[color] ?? 'bg-gray-50 dark:bg-gray-800/40')
  const borderClass = isLead
    ? 'border-amber-300 dark:border-amber-700'
    : (BORDER_COLOR_MAP[color] ?? 'border-gray-300 dark:border-gray-600')

  return (
    <div className={cn('px-5', showHeader ? 'pt-5 pb-1' : 'py-1')}>
      <div className="flex items-start gap-3">
        {avatar}
        <div className="flex-1 min-w-0">
          {header}
          {/* Bubble */}
          <div
            className={cn(
              'rounded-2xl px-4 py-3 border-l-3',
              bgClass,
              borderClass,
              !expanded && isLong && 'max-h-[240px] overflow-hidden relative',
            )}
          >
            <div className="text-sm text-gray-800 dark:text-gray-200 prose prose-sm dark:prose-invert max-w-none leading-relaxed">
              <Markdown components={markdownComponents}>{msg.text}</Markdown>
            </div>
            {!expanded && isLong && (
              <div className="absolute bottom-0 left-0 right-0 h-16 bg-gradient-to-t from-white/90 dark:from-gray-950/90 to-transparent pointer-events-none rounded-b-2xl" />
            )}
          </div>
          {isLong && (
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="text-xs font-medium text-blue-600 dark:text-blue-400 hover:text-blue-800 dark:hover:text-blue-200 mt-2"
            >
              {expanded ? 'Show less' : 'Show more'}
            </button>
          )}
        </div>
      </div>
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
  /** Sidechain data — when provided, completion badges are interleaved in the timeline. */
  sidechains?: TeamMemberSidechain[]
  /** Click handler for sidechain badges — opens the JSONL drill-down view. */
  onSidechainSelect?: (target: { hexId: string; memberName: string }) => void
}

export function TeamChatView({
  messages,
  members,
  topic,
  sidechains,
  onSidechainSelect,
}: TeamChatViewProps) {
  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [atBottom, setAtBottom] = useState(true)

  // Build member lookup
  const memberMap = new Map(members.map((m) => [m.name, m]))

  // Merge messages + sidechain completions into a single sorted timeline
  const timeline = useMemo(() => buildTimeline(messages, sidechains), [messages, sidechains])
  const prevCountRef = useRef(timeline.length)

  // Auto-scroll when new items arrive (only if user was at bottom)
  useEffect(() => {
    if (timeline.length > prevCountRef.current && atBottom) {
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({
          index: timeline.length - 1,
          align: 'end',
          behavior: 'smooth',
        })
      })
    }
    prevCountRef.current = timeline.length
  }, [timeline.length, atBottom])

  const handleAtBottomChange = useCallback((bottom: boolean) => {
    setAtBottom(bottom)
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: timeline.length - 1,
      align: 'end',
      behavior: 'smooth',
    })
  }, [timeline.length])

  if (timeline.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <p className="text-xs text-gray-500 dark:text-gray-400">No messages yet</p>
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full">
      {/* Pinned header: topic + speakers */}
      {(topic || members.length > 0) && (
        <div className="flex-shrink-0 px-5 py-3 border-b border-gray-200 dark:border-gray-800 bg-gray-50/50 dark:bg-gray-900/50">
          {topic && (
            <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 leading-snug mb-2">
              {topic}
            </h3>
          )}
          <div className="flex flex-wrap gap-3">
            {members.map((m) => {
              const isLead = m.agentType === 'team-lead'
              return (
                <div key={m.agentId} className="flex items-center gap-1.5">
                  {isLead ? (
                    <Crown className="w-3.5 h-3.5 shrink-0 text-amber-500 dark:text-amber-400" />
                  ) : (
                    <span
                      className={cn(
                        'w-2.5 h-2.5 rounded-full shrink-0',
                        DOT_COLOR_MAP[m.color] ?? 'bg-gray-400',
                      )}
                    />
                  )}
                  <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
                    {m.name}
                  </span>
                  <span className="text-xs text-gray-400 dark:text-gray-500">
                    {formatModelName(m.model)}
                  </span>
                </div>
              )
            })}
          </div>
        </div>
      )}

      {/* Chat messages */}
      <div className="flex-1 min-h-0 relative">
        <Virtuoso
          ref={virtuosoRef}
          data={timeline}
          alignToBottom
          followOutput="smooth"
          atBottomStateChange={handleAtBottomChange}
          atBottomThreshold={60}
          itemContent={(index, item) => {
            if (item.kind === 'work-done') {
              return (
                <WorkDoneBadge
                  sidechain={item.sidechain}
                  member={memberMap.get(item.sidechain.memberName)}
                  onSelect={onSidechainSelect}
                />
              )
            }

            const msg = item.msg
            // Look back for previous message item (skip badge items)
            let prev: InboxMessage | undefined
            for (let i = index - 1; i >= 0; i--) {
              const p = timeline[i]
              if (p.kind === 'message') {
                prev = p.msg
                break
              }
            }
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
