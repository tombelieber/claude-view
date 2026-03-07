import { useMemo } from 'react'
import { cn } from '../../lib/utils'
import { KanbanGroupByControl } from './KanbanGroupByControl'
import { BranchHeader, ProjectHeader } from './KanbanSwimLaneHeader'
import { SessionCard } from './SessionCard'
import type { AgentStateGroup, KanbanGroupBy } from './types'
import type { ProjectGroup } from './use-kanban-grouping'
import { branchCollapseKey, projectCollapseKey } from './use-kanban-grouping'
import type { LiveSession } from './use-live-sessions'

interface KanbanViewProps {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  // Grouping
  groupBy: KanbanGroupBy
  onGroupByChange: (value: KanbanGroupBy) => void
  projectGroups: ProjectGroup[]
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
}

/** Sort key for needs_you sessions: urgency ordering */
export function needsYouSortKey(session: LiveSession): number {
  switch (session.agentState.state) {
    case 'needs_permission':
      return 0
    case 'awaiting_input':
      return 1
    case 'interrupted':
      return 2
    case 'error':
      return 3
    case 'awaiting_approval':
      return 4
    case 'idle':
      return 5
    default:
      return 6
  }
}

function sortNeedsYou(sessions: LiveSession[]): LiveSession[] {
  return [...sessions].sort((a, b) => {
    const cacheRank = (s: LiveSession) =>
      s.cacheStatus === 'warm' ? 0 : s.cacheStatus === 'unknown' ? 1 : 2
    const cacheDiff = cacheRank(a) - cacheRank(b)
    if (cacheDiff !== 0) return cacheDiff
    const keyDiff = needsYouSortKey(a) - needsYouSortKey(b)
    if (keyDiff !== 0) return keyDiff
    return b.lastActivityAt - a.lastActivityAt
  })
}

function sortAutonomous(sessions: LiveSession[]): LiveSession[] {
  return [...sessions].sort((a, b) => {
    const aTime = a.currentTurnStartedAt ?? a.lastActivityAt
    const bTime = b.currentTurnStartedAt ?? b.lastActivityAt
    return bTime - aTime
  })
}

function splitByGroup(sessions: LiveSession[]): Record<AgentStateGroup, LiveSession[]> {
  const groups: Record<AgentStateGroup, LiveSession[]> = { needs_you: [], autonomous: [] }
  for (const s of sessions) {
    groups[s.agentState.group].push(s)
  }
  groups.needs_you = sortNeedsYou(groups.needs_you)
  groups.autonomous = sortAutonomous(groups.autonomous)
  return groups
}

// --- Urgent sub-count (preserved from KanbanColumn) ---

function NeedsYouSubCount({ sessions }: { sessions: LiveSession[] }) {
  const urgent = sessions.filter(
    (s) => s.agentState.state === 'awaiting_input' || s.agentState.state === 'needs_permission',
  ).length
  if (urgent === 0) return null
  return <span className="ml-1.5 text-[10px] text-amber-500 font-normal">{urgent} urgent</span>
}

// --- Column header ---

function ColumnHeaders({
  needsYouCount,
  autonomousCount,
  needsYouSessions,
}: {
  needsYouCount: number
  autonomousCount: number
  needsYouSessions: LiveSession[]
}) {
  return (
    <div className="flex gap-4 flex-shrink-0">
      <div className="flex-1 min-w-0">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-amber-500" />
          <div className="px-3 py-2 flex items-center justify-between">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
              Needs You
              <NeedsYouSubCount sessions={needsYouSessions} />
            </span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({needsYouCount})</span>
          </div>
        </div>
      </div>
      <div className="flex-1 min-w-0">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-green-500" />
          <div className="px-3 py-2 flex items-center justify-between">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Running</span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({autonomousCount})</span>
          </div>
        </div>
      </div>
    </div>
  )
}

// --- Card list for one column side ---

function CardSlot({
  sessions,
  group,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  hideProjectBranch,
}: {
  sessions: LiveSession[]
  group: AgentStateGroup
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  hideProjectBranch: boolean
}) {
  if (sessions.length === 0) return <div className="min-h-[1px]" />

  return (
    <div className="space-y-3">
      {sessions.map((session) => (
        <div
          key={session.id}
          data-session-id={session.id}
          onClick={() => onSelect(session.id)}
          className={cn(
            'cursor-pointer rounded-lg transition-opacity',
            session.id === selectedId && 'ring-2 ring-indigo-500 rounded-lg',
            group === 'needs_you' && session.cacheStatus !== 'warm' && 'opacity-70',
          )}
        >
          <SessionCard
            session={session}
            stalledSessions={stalledSessions}
            currentTime={currentTime}
            onClickOverride={onCardClick ? () => onCardClick(session.id) : undefined}
            hideProjectBranch={hideProjectBranch}
          />
        </div>
      ))}
    </div>
  )
}

// --- Two-column card row for a branch ---

function BranchCardRow({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  hideProjectBranch,
}: {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  hideProjectBranch: boolean
}) {
  const split = useMemo(() => splitByGroup(sessions), [sessions])

  return (
    <div className="flex gap-4 px-3 py-2">
      <div className="flex-1 min-w-0">
        <CardSlot
          sessions={split.needs_you}
          group="needs_you"
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
          hideProjectBranch={hideProjectBranch}
        />
      </div>
      <div className="flex-1 min-w-0">
        <CardSlot
          sessions={split.autonomous}
          group="autonomous"
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
          hideProjectBranch={hideProjectBranch}
        />
      </div>
    </div>
  )
}

// --- Flat view (no grouping) — preserves current behavior ---

function FlatKanban({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}) {
  const split = useMemo(() => splitByGroup(sessions), [sessions])

  return (
    <>
      <ColumnHeaders
        needsYouCount={split.needs_you.length}
        autonomousCount={split.autonomous.length}
        needsYouSessions={split.needs_you}
      />
      <div className="flex gap-4 flex-1 min-h-0">
        <div className="flex flex-col flex-1 min-w-0 h-full min-h-0">
          <div className="space-y-3 p-3 flex-1 min-h-0 overflow-y-auto">
            {split.needs_you.length === 0 ? (
              <p className="text-xs text-gray-400 dark:text-gray-500 py-8 text-center">
                No sessions need attention
              </p>
            ) : (
              <CardSlot
                sessions={split.needs_you}
                group="needs_you"
                selectedId={selectedId}
                onSelect={onSelect}
                stalledSessions={stalledSessions}
                currentTime={currentTime}
                onCardClick={onCardClick}
                hideProjectBranch={false}
              />
            )}
          </div>
        </div>
        <div className="flex flex-col flex-1 min-w-0 h-full min-h-0">
          <div className="space-y-3 p-3 flex-1 min-h-0 overflow-y-auto">
            {split.autonomous.length === 0 ? (
              <p className="text-xs text-gray-400 dark:text-gray-500 py-8 text-center">
                No autonomous sessions
              </p>
            ) : (
              <CardSlot
                sessions={split.autonomous}
                group="autonomous"
                selectedId={selectedId}
                onSelect={onSelect}
                stalledSessions={stalledSessions}
                currentTime={currentTime}
                onCardClick={onCardClick}
                hideProjectBranch={false}
              />
            )}
          </div>
        </div>
      </div>
    </>
  )
}

// --- Grouped view (swimlanes) ---

function GroupedKanban({
  sessions,
  projectGroups,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  isCollapsed,
  toggleCollapse,
}: {
  sessions: LiveSession[]
  projectGroups: ProjectGroup[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
}) {
  // Count totals for column headers
  const needsYouSessions = sessions.filter((s) => s.agentState.group === 'needs_you')
  const autonomousCount = sessions.filter((s) => s.agentState.group === 'autonomous').length

  return (
    <>
      <ColumnHeaders
        needsYouCount={needsYouSessions.length}
        autonomousCount={autonomousCount}
        needsYouSessions={needsYouSessions}
      />
      <div className="flex-1 min-h-0 overflow-y-auto">
        {projectGroups.map((project) => {
          const projKey = projectCollapseKey(project.projectName)
          const projCollapsed = isCollapsed(projKey)

          return (
            <div key={project.projectName} className="mb-2">
              <ProjectHeader
                projectName={project.projectName}
                totalCostUsd={project.totalCostUsd}
                sessionCount={project.totalSessionCount}
                isCollapsed={projCollapsed}
                onToggle={() => toggleCollapse(projKey)}
              />

              {!projCollapsed &&
                project.branches.map((branch) => {
                  const brKey = branchCollapseKey(project.projectName, branch.branchName)
                  const brCollapsed = isCollapsed(brKey)

                  return (
                    <div key={branch.branchName ?? '__null__'}>
                      <BranchHeader
                        branchName={branch.branchName}
                        sessionCount={branch.sessionCount}
                        isCollapsed={brCollapsed}
                        onToggle={() => toggleCollapse(brKey)}
                      />
                      {!brCollapsed && (
                        <BranchCardRow
                          sessions={branch.sessions}
                          selectedId={selectedId}
                          onSelect={onSelect}
                          stalledSessions={stalledSessions}
                          currentTime={currentTime}
                          onCardClick={onCardClick}
                          hideProjectBranch={true}
                        />
                      )}
                    </div>
                  )
                })}
            </div>
          )
        })}

        {projectGroups.length === 0 && (
          <p className="text-xs text-gray-400 dark:text-gray-500 py-8 text-center">
            No active sessions
          </p>
        )}
      </div>
    </>
  )
}

// --- Main export ---

export function KanbanView({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  groupBy,
  onGroupByChange,
  projectGroups,
  isCollapsed,
  toggleCollapse,
}: KanbanViewProps) {
  return (
    <div className="flex flex-col h-full min-h-0 gap-2 pb-4">
      <KanbanGroupByControl value={groupBy} onChange={onGroupByChange} />

      {groupBy === 'none' ? (
        <FlatKanban
          sessions={sessions}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
        />
      ) : (
        <GroupedKanban
          sessions={sessions}
          projectGroups={projectGroups}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
          isCollapsed={isCollapsed}
          toggleCollapse={toggleCollapse}
        />
      )}
    </div>
  )
}
