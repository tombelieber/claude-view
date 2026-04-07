import { useMemo, useState } from 'react'
import { useTeamDetail, useTeamInbox, useTeamSidechains } from '../../hooks/use-teams'
import type { TeamMember } from '../../types/generated'
import type { TeamMemberSidechain } from '@claude-view/shared/types/generated/TeamMemberSidechain'
import type { TeamTranscriptBlock } from '../../types/generated/TeamTranscriptBlock'
import { TeamChatView } from './TeamChatView'
import { TranscriptHeader } from './TranscriptHeader'
import { TranscriptBody } from './TranscriptBody'
import { SubAgentBlockView } from '../live/SubAgentBlockView'

interface TeamsTabProps {
  teamName: string
  sessionId: string
  inboxVersion?: number
  transcript?: TeamTranscriptBlock | null
  /** SSE-pushed members for live sessions — renders directly, zero HTTP calls. */
  sseMembers?: TeamMember[]
}

// ============================================================================
// Main component
// ============================================================================

export function TeamsTab({
  teamName,
  sessionId,
  inboxVersion,
  transcript,
  sseMembers,
}: TeamsTabProps) {
  const hasSseMembers = sseMembers && sseMembers.length > 0
  const [drillDown, setDrillDown] = useState<{ hexId: string; memberName: string } | null>(null)

  // API hooks — disabled when SSE provides members (live), or when transcript view is active
  const { data: team, isLoading: teamLoading } = useTeamDetail(
    transcript || hasSseMembers ? null : teamName,
  )
  // Inbox: inboxVersion comes from SSE teamInboxCount — query key changes → auto-refetch
  const { data: inbox, isLoading: inboxLoading } = useTeamInbox(
    transcript ? null : teamName,
    inboxVersion,
  )
  const { data: sidechains } = useTeamSidechains(teamName, sessionId, inboxVersion)

  const sidechainsByMember = useMemo(() => {
    if (!sidechains?.length) return new Map<string, TeamMemberSidechain[]>()
    const map = new Map<string, TeamMemberSidechain[]>()
    for (const sc of sidechains) {
      const list = map.get(sc.memberName) ?? []
      list.push(sc)
      map.set(sc.memberName, list)
    }
    return map
  }, [sidechains])

  // Drill-down: render SubAgentBlockView instead of inbox
  if (drillDown) {
    return (
      <SubAgentBlockView
        sessionId={sessionId}
        agentId={drillDown.hexId}
        agentType={drillDown.memberName}
        description={`${drillDown.memberName} sidechain`}
        onClose={() => setDrillDown(null)}
      />
    )
  }

  // If we have a transcript block (from JSONL accumulation), render the clean view
  if (transcript) {
    const speakerMap = new Map(
      transcript.speakers.map((s) => [s.id, { displayName: s.displayName, color: s.color }]),
    )
    return (
      <div className="p-4 overflow-y-auto h-full">
        <TranscriptHeader topic={transcript.description} speakers={transcript.speakers} />
        <TranscriptBody entries={transcript.entries} speakers={speakerMap} />
      </div>
    )
  }

  // SSE members available → render directly, zero HTTP latency
  const members = hasSseMembers ? sseMembers : team?.members
  const isLoading = hasSseMembers ? false : teamLoading

  if (isLoading || inboxLoading) {
    return (
      <div className="p-4 space-y-3 animate-pulse">
        <div className="h-12 rounded bg-gray-100 dark:bg-gray-800" />
        <div className="h-32 rounded bg-gray-100 dark:bg-gray-800" />
        <div className="h-48 rounded bg-gray-100 dark:bg-gray-800" />
      </div>
    )
  }

  if (!members || members.length === 0) {
    return (
      <div className="p-4 space-y-2">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100">{teamName}</h3>
        <p className="text-xs text-gray-500 dark:text-gray-400">
          Team data is no longer available. The team may have been disbanded after the session
          ended.
        </p>
      </div>
    )
  }

  // Primary view: group chat (left) + member sessions sidebar (right)
  return (
    <div className="flex h-full overflow-hidden">
      <div className="flex-1 min-w-0">
        <TeamChatView
          messages={inbox ?? []}
          members={members}
          topic={team?.description ?? teamName}
        />
      </div>
      {sidechainsByMember.size > 0 && (
        <SidechainsSection byMember={sidechainsByMember} onSelect={setDrillDown} />
      )}
    </div>
  )
}

// ============================================================================
// Sidechains section
// ============================================================================

function SidechainsSection({
  byMember,
  onSelect,
}: {
  byMember: Map<string, TeamMemberSidechain[]>
  onSelect: (target: { hexId: string; memberName: string }) => void
}) {
  return (
    <div className="w-56 flex-shrink-0 border-l border-gray-200 dark:border-gray-800 px-3 py-2 overflow-y-auto">
      <h4 className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-1.5">
        Member Sessions
      </h4>
      {[...byMember.entries()].map(([member, chains]) => (
        <div key={member} className="mb-2 last:mb-0">
          <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-0.5">
            {member} <span className="text-gray-400 dark:text-gray-500">({chains.length})</span>
          </p>
          {chains.map((sc) => (
            <button
              key={sc.hexId}
              type="button"
              onClick={() => onSelect({ hexId: sc.hexId, memberName: sc.memberName })}
              className="w-full flex items-center gap-2 px-2 py-1 rounded text-left hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            >
              <span className="font-mono text-xs text-gray-500 dark:text-gray-400">
                {sc.hexId.slice(0, 8)}
              </span>
              <span className="text-xs text-gray-600 dark:text-gray-400">{sc.lineCount} lines</span>
              <span className="text-xs text-gray-400 dark:text-gray-500">
                {(sc.fileSizeBytes / 1024).toFixed(1)} KB
              </span>
            </button>
          ))}
        </div>
      ))}
    </div>
  )
}
