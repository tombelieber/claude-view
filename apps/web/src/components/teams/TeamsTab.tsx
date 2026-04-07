import { CheckCircle2, ChevronRight } from 'lucide-react'
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

/** Format seconds into a compact human-readable duration (e.g., "21m", "3m 12s", "34s"). */
function formatCompactDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  const m = Math.floor(seconds / 60)
  const s = seconds % 60
  return s > 0 ? `${m}m ${s}s` : `${m}m`
}

/** Shorten model ID for display (e.g., "claude-opus-4-6" → "opus"). */
function shortModel(model: string): string {
  if (!model) return ''
  // Extract the model family name: "claude-opus-4-6" → "opus", "claude-haiku-4-5-20251001" → "haiku"
  const match = model.match(/claude-(\w+)-/)
  return match ? match[1] : model
}

function SidechainsSection({
  byMember,
  onSelect,
}: {
  byMember: Map<string, TeamMemberSidechain[]>
  onSelect: (target: { hexId: string; memberName: string }) => void
}) {
  return (
    <div className="w-56 flex-shrink-0 border-l border-gray-200 dark:border-gray-800 overflow-y-auto">
      <h4 className="px-3 pt-2 pb-1 text-[10px] font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider">
        Member Sessions
      </h4>

      {[...byMember.entries()].map(([member, chains]) => {
        const model = shortModel(chains[0]?.model ?? '')
        return (
          <div key={member} className="px-3 py-1.5 border-b border-gray-100 dark:border-gray-800/50 last:border-b-0">
            {/* Member header: name + model badge + session count */}
            <p className="text-xs font-medium text-gray-700 dark:text-gray-300 mb-0.5">
              {member}
            </p>
            <div className="flex items-center gap-1.5 mb-1.5">
              {model && (
                <span className="inline-flex items-center px-1 py-px rounded text-[10px] font-medium bg-sky-50 text-sky-600 dark:bg-sky-900/30 dark:text-sky-400">
                  {model}
                </span>
              )}
              <span className="text-[10px] text-gray-400 dark:text-gray-500">
                {chains.length} {chains.length === 1 ? 'session' : 'sessions'}
              </span>
            </div>

            {/* Sidechain rows */}
            {chains.map((sc) => {
              const isShort = sc.durationSeconds < 60
              return (
                <button
                  key={sc.hexId}
                  type="button"
                  onClick={() => onSelect({ hexId: sc.hexId, memberName: sc.memberName })}
                  className="group w-full flex items-center gap-1.5 px-1.5 py-1 rounded text-left hover:bg-gray-50 dark:hover:bg-gray-800/60 transition-colors"
                >
                  {/* Status indicator */}
                  <CheckCircle2 className={`w-3 h-3 flex-shrink-0 ${
                    isShort
                      ? 'text-amber-400 dark:text-amber-500'
                      : 'text-green-500 dark:text-green-400'
                  }`} />
                  {/* Duration (hero metric) */}
                  <span className="text-xs tabular-nums text-gray-600 dark:text-gray-400 min-w-[2.5rem]">
                    {formatCompactDuration(sc.durationSeconds)}
                  </span>
                  {/* Line count */}
                  <span className="text-[10px] text-gray-400 dark:text-gray-500">
                    {sc.lineCount} lines
                  </span>
                  {/* Drill-down chevron */}
                  <ChevronRight className="w-3 h-3 ml-auto flex-shrink-0 text-gray-300 dark:text-gray-600 opacity-0 group-hover:opacity-100 transition-opacity" />
                </button>
              )
            })}
          </div>
        )
      })}
    </div>
  )
}
