import { useTeamCost, useTeamDetail, useTeamInbox } from '../../hooks/use-teams'
import type { TeamMember } from '../../types/generated'
import type { TeamTranscriptBlock } from '../../types/generated/TeamTranscriptBlock'
import { TeamBudgetSection } from './TeamBudgetSection'
import { TeamChatView } from './TeamChatView'
import { TranscriptHeader } from './TranscriptHeader'
import { TranscriptBody } from './TranscriptBody'

interface TeamsTabProps {
  teamName: string
  inboxVersion?: number
  transcript?: TeamTranscriptBlock | null
  /** SSE-pushed members for live sessions — renders directly, zero HTTP calls. */
  sseMembers?: TeamMember[]
}

// ============================================================================
// Main component
// ============================================================================

export function TeamsTab({ teamName, inboxVersion, transcript, sseMembers }: TeamsTabProps) {
  const hasSseMembers = sseMembers && sseMembers.length > 0

  // API hooks — disabled when SSE provides members (live), or when transcript view is active
  const { data: team, isLoading: teamLoading } = useTeamDetail(
    transcript || hasSseMembers ? null : teamName,
  )
  // Inbox: inboxVersion comes from SSE teamInboxCount — query key changes → auto-refetch
  const { data: inbox, isLoading: inboxLoading } = useTeamInbox(
    transcript ? null : teamName,
    inboxVersion,
  )
  const { data: teamCost } = useTeamCost(transcript ? null : teamName)

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

  // Primary view: group chat with team cost footer
  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 min-h-0">
        <TeamChatView
          messages={inbox ?? []}
          members={members}
          topic={team?.description ?? teamName}
        />
      </div>
      {teamCost && (
        <div className="flex-shrink-0 border-t border-gray-200 dark:border-gray-800 p-3">
          <TeamBudgetSection cost={teamCost} />
        </div>
      )}
    </div>
  )
}
