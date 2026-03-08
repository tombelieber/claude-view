import { ChevronDown, ChevronRight, Crown } from 'lucide-react'
import { useState } from 'react'
import Markdown from 'react-markdown'
import { useTeamDetail, useTeamInbox } from '../../hooks/use-teams'
import { cn } from '../../lib/utils'
import type { InboxMessage, InboxMessageType, TeamMember } from '../../types/generated'

interface TeamsTabProps {
  teamName: string
}

// ============================================================================
// Sub-components
// ============================================================================

const COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-yellow-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

function MemberRow({ member, isLead }: { member: TeamMember; isLead: boolean }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="py-2">
      <div className="flex items-center gap-2">
        <span
          className={cn('w-2.5 h-2.5 rounded-full', COLOR_MAP[member.color] || 'bg-gray-400')}
        />
        {isLead && <Crown className="w-3 h-3 text-yellow-500 shrink-0" />}
        <span className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
          {member.name}
        </span>
        <span className="text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500">
          {member.model}
        </span>
        <span className="text-[10px] text-gray-400">{member.agentType}</span>
      </div>
      {member.prompt && (
        <div className="ml-5 mt-1">
          <button
            type="button"
            onClick={() => setExpanded(!expanded)}
            className="flex items-center gap-1 text-[11px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            task prompt
          </button>
          {expanded && (
            <p className="mt-1 text-xs text-gray-500 dark:text-gray-400 whitespace-pre-wrap border-l-2 border-gray-200 dark:border-gray-700 pl-2">
              {member.prompt}
            </p>
          )}
        </div>
      )}
    </div>
  )
}

function isProtocolMessage(type: InboxMessageType): boolean {
  return type === 'idleNotification' || type === 'shutdownRequest' || type === 'shutdownApproved'
}

function MessageItem({ msg }: { msg: InboxMessage }) {
  const [expanded, setExpanded] = useState(false)
  const isProtocol = isProtocolMessage(msg.messageType)
  const isLong = msg.text.length > 400

  if (isProtocol) {
    // Protocol messages: single dimmed line
    const label =
      msg.messageType === 'idleNotification'
        ? 'idle'
        : msg.messageType === 'shutdownRequest'
          ? 'shutdown request'
          : 'shutdown approved'
    return (
      <div className="flex items-center gap-2 py-1 text-[11px] text-gray-300 dark:text-gray-600">
        <span
          className={cn('w-1.5 h-1.5 rounded-full', COLOR_MAP[msg.color ?? ''] || 'bg-gray-300')}
        />
        <span>{msg.from}</span>
        <span className="italic">{label}</span>
        <span className="ml-auto">{msg.timestamp.slice(11, 16)}</span>
      </div>
    )
  }

  return (
    <div className="py-2">
      <div className="flex items-center gap-2 mb-1">
        <span className={cn('w-2 h-2 rounded-full', COLOR_MAP[msg.color ?? ''] || 'bg-gray-400')} />
        <span className="text-xs font-medium text-gray-700 dark:text-gray-300">{msg.from}</span>
        <span className="ml-auto text-[10px] text-gray-400">{msg.timestamp.slice(11, 16)}</span>
      </div>
      <div
        className={cn(
          'text-xs text-gray-600 dark:text-gray-400 prose prose-xs dark:prose-invert max-w-none',
          'bg-gray-50 dark:bg-gray-800/50 rounded-md p-2',
          !expanded && isLong && 'line-clamp-6',
        )}
      >
        <Markdown>{msg.text}</Markdown>
      </div>
      {isLong && (
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="text-[11px] text-blue-500 hover:text-blue-600 mt-1"
        >
          {expanded ? 'collapse' : `expand (${msg.text.length.toLocaleString()} chars)`}
        </button>
      )}
    </div>
  )
}

// ============================================================================
// Main component
// ============================================================================

export function TeamsTab({ teamName }: TeamsTabProps) {
  const { data: team, isLoading: teamLoading } = useTeamDetail(teamName)
  const { data: inbox, isLoading: inboxLoading } = useTeamInbox(teamName)

  if (teamLoading || inboxLoading) {
    return (
      <div className="p-4 space-y-3 animate-pulse">
        <div className="h-12 rounded bg-gray-100 dark:bg-gray-800" />
        <div className="h-32 rounded bg-gray-100 dark:bg-gray-800" />
        <div className="h-48 rounded bg-gray-100 dark:bg-gray-800" />
      </div>
    )
  }

  if (!team) return null

  const leadName = team.members.find((m) => m.agentType === 'team-lead')?.name

  return (
    <div className="p-4 overflow-y-auto h-full space-y-4">
      {/* Header */}
      <div>
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{team.name}</h3>
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{team.description}</p>
        <p className="text-[10px] text-gray-400 mt-1">
          Created {new Date(team.createdAt).toLocaleString()}
        </p>
      </div>

      {/* Members */}
      <div>
        <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">
          Members ({team.members.length})
        </h4>
        <div className="divide-y divide-gray-100 dark:divide-gray-800">
          {team.members.map((m) => (
            <MemberRow key={m.agentId} member={m} isLead={m.name === leadName} />
          ))}
        </div>
      </div>

      {/* Inbox */}
      {inbox && inbox.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wider mb-1">
            Inbox ({inbox.length} messages)
          </h4>
          <div className="space-y-0.5">
            {inbox.map((msg, i) => (
              <MessageItem key={`${msg.from}-${msg.timestamp}-${i}`} msg={msg} />
            ))}
          </div>
        </div>
      )}
    </div>
  )
}
