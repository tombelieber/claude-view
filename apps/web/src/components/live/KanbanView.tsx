import { Copy, X } from 'lucide-react'
import { useMemo } from 'react'
import { formatRelativeTime } from '../../lib/format-utils'
import { cn } from '../../lib/utils'
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
  projectGroups: ProjectGroup[]
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
  // Recently closed
  recentlyClosed?: LiveSession[]
  onDismiss?: (sessionId: string) => void
  onDismissAll?: () => void
  showClosed?: boolean
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
  const groups: Record<AgentStateGroup, LiveSession[]> = {
    needs_you: [],
    autonomous: [],
  }
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
  return <span className="ml-1.5 text-xs text-amber-500 font-normal">{urgent} urgent</span>
}

// --- Column headers ---

function ColumnHeaders({
  needsYouCount,
  autonomousCount,
  needsYouSessions,
  closedCount,
  showClosed,
  onDismissAll,
}: {
  needsYouCount: number
  autonomousCount: number
  needsYouSessions: LiveSession[]
  closedCount: number
  showClosed: boolean
  onDismissAll?: () => void
}) {
  return (
    <div className="flex gap-4 flex-shrink-0">
      <div className="flex-1 min-w-0">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-amber-500" />
          <div className="px-3 py-2 flex items-center gap-1.5">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Needs You</span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({needsYouCount})</span>
            <NeedsYouSubCount sessions={needsYouSessions} />
          </div>
        </div>
      </div>
      <div className="flex-1 min-w-0">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-green-500" />
          <div className="px-3 py-2 flex items-center gap-1.5">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Running</span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({autonomousCount})</span>
          </div>
        </div>
      </div>
      {showClosed && (
        <div className="flex-1 min-w-0">
          <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
            <div className="h-0.5 rounded-t-lg bg-zinc-400 dark:bg-zinc-600" />
            <div className="px-3 py-2 flex items-center gap-1.5">
              <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Closed</span>
              <span className="text-xs text-gray-400 dark:text-gray-500">({closedCount})</span>
              {closedCount > 0 && onDismissAll && (
                <button
                  type="button"
                  onClick={onDismissAll}
                  className="ml-auto text-xs text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300 cursor-pointer transition-colors"
                >
                  Dismiss All
                </button>
              )}
            </div>
          </div>
        </div>
      )}
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
          role="button"
          tabIndex={0}
          data-session-id={session.id}
          onClick={() => onSelect(session.id)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              onSelect(session.id)
            }
          }}
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

// --- Closed card slot — dimmed cards with dismiss/resume overlays ---

function ClosedCardSlot({
  sessions,
  selectedId,
  onSelect,
  currentTime,
  onDismiss,
  hideProjectBranch,
}: {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  currentTime: number
  onDismiss?: (sessionId: string) => void
  hideProjectBranch: boolean
}) {
  if (sessions.length === 0) return <div className="min-h-[1px]" />

  return (
    <div className="space-y-3">
      {sessions.map((session) => (
        <div key={session.id} className="relative group opacity-60">
          {/* Dismiss button */}
          {onDismiss && (
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation()
                onDismiss(session.id)
              }}
              className="absolute top-2 right-2 z-10 p-0.5 rounded hover:bg-zinc-200 dark:hover:bg-zinc-700 opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
              title="Dismiss"
            >
              <X className="w-3.5 h-3.5 text-zinc-400" />
            </button>
          )}
          {/* Resume button */}
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation()
              navigator.clipboard.writeText(`claude --resume ${session.id}`).catch(() => {})
            }}
            className="absolute bottom-2 left-2 z-10 flex items-center gap-1 px-1.5 py-0.5 text-xs text-zinc-500 dark:text-zinc-400 hover:text-zinc-700 dark:hover:text-zinc-200 bg-zinc-100 dark:bg-zinc-800 rounded border border-zinc-200 dark:border-zinc-700 opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
            title="Copy resume command"
          >
            <Copy className="w-3 h-3" />
            Resume
          </button>
          {/* Closed-time label */}
          {session.closedAt && (
            <div className="absolute bottom-2 right-2 z-10 text-xs text-zinc-400">
              closed {formatRelativeTime(session.closedAt)}
            </div>
          )}
          <div
            className={cn(
              'border border-dashed border-zinc-300 dark:border-zinc-700 rounded-lg cursor-pointer',
              session.id === selectedId && 'ring-2 ring-indigo-500',
            )}
            onClick={() => onSelect(session.id)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') onSelect(session.id)
            }}
            role="button"
            tabIndex={0}
          >
            <SessionCard
              session={session}
              currentTime={currentTime}
              onClickOverride={() => onSelect(session.id)}
              hideProjectBranch={hideProjectBranch}
            />
          </div>
        </div>
      ))}
    </div>
  )
}

// --- Three-column card row for a branch ---

function BranchCardRow({
  sessions,
  closedSessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  hideProjectBranch,
  showClosed,
  onDismiss,
}: {
  sessions: LiveSession[]
  closedSessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  hideProjectBranch: boolean
  showClosed: boolean
  onDismiss?: (sessionId: string) => void
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
      {showClosed && (
        <div className="flex-1 min-w-0">
          <ClosedCardSlot
            sessions={closedSessions}
            selectedId={selectedId}
            onSelect={onSelect}
            currentTime={currentTime}
            onDismiss={onDismiss}
            hideProjectBranch={hideProjectBranch}
          />
        </div>
      )}
    </div>
  )
}

// --- Flat view (no grouping) ---

function FlatKanban({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  recentlyClosed,
  showClosed,
  onDismiss,
  onDismissAll,
}: {
  sessions: LiveSession[]
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  recentlyClosed: LiveSession[]
  showClosed: boolean
  onDismiss?: (sessionId: string) => void
  onDismissAll?: () => void
}) {
  const split = useMemo(() => splitByGroup(sessions), [sessions])

  return (
    <>
      <ColumnHeaders
        needsYouCount={split.needs_you.length}
        autonomousCount={split.autonomous.length}
        needsYouSessions={split.needs_you}
        closedCount={recentlyClosed.length}
        showClosed={showClosed}
        onDismissAll={onDismissAll}
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
        {showClosed && (
          <div className="flex flex-col flex-1 min-w-0 h-full min-h-0">
            <div className="space-y-3 p-3 flex-1 min-h-0 overflow-y-auto">
              {recentlyClosed.length === 0 ? (
                <p className="text-xs text-gray-400 dark:text-gray-500 py-8 text-center">
                  No recently closed sessions
                </p>
              ) : (
                <ClosedCardSlot
                  sessions={recentlyClosed}
                  selectedId={selectedId}
                  onSelect={onSelect}
                  currentTime={currentTime}
                  onDismiss={onDismiss}
                  hideProjectBranch={false}
                />
              )}
            </div>
          </div>
        )}
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
  recentlyClosed,
  showClosed,
  onDismiss,
  onDismissAll,
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
  recentlyClosed: LiveSession[]
  showClosed: boolean
  onDismiss?: (sessionId: string) => void
  onDismissAll?: () => void
}) {
  // Count totals for column headers
  const needsYouSessions = sessions.filter((s) => s.agentState.group === 'needs_you')
  const autonomousCount = sessions.filter((s) => s.agentState.group === 'autonomous').length

  // Build a merged view: projectGroups + closed sessions placed into matching swimlanes.
  // Closed sessions whose project/branch doesn't exist yet get their own swimlane rows.
  const { mergedProjects, closedByKey } = useMemo(() => {
    // Index closed sessions by project::branch
    const byKey = new Map<string, LiveSession[]>()
    const byProject = new Map<string, Map<string, LiveSession[]>>()
    for (const s of recentlyClosed) {
      const projectKey = s.projectDisplayName || s.project
      const branchKey = s.effectiveBranch ?? '__null__'
      const flatKey = `${projectKey}::${branchKey}`
      const arr = byKey.get(flatKey)
      if (arr) {
        arr.push(s)
      } else {
        byKey.set(flatKey, [s])
      }
      // Also index by project for branch discovery
      let branchMap = byProject.get(projectKey)
      if (!branchMap) {
        branchMap = new Map()
        byProject.set(projectKey, branchMap)
      }
      let branchArr = branchMap.get(branchKey)
      if (!branchArr) {
        branchArr = []
        branchMap.set(branchKey, branchArr)
      }
      branchArr.push(s)
    }

    // Track which project::branch combos are already covered
    const coveredKeys = new Set<string>()
    const coveredProjects = new Set<string>()

    // Start with existing projectGroups (which only have active sessions)
    const merged: typeof projectGroups = projectGroups.map((project) => {
      coveredProjects.add(project.projectName)
      const branches = project.branches.map((branch) => {
        coveredKeys.add(`${project.projectName}::${branch.branchName ?? '__null__'}`)
        return branch
      })

      // Add closed-only branches for this project
      const closedBranches = byProject.get(project.projectName)
      if (closedBranches) {
        for (const [branchKey] of closedBranches) {
          const flatKey = `${project.projectName}::${branchKey}`
          if (!coveredKeys.has(flatKey)) {
            coveredKeys.add(flatKey)
            // Create a branch entry with no active sessions (closed-only)
            branches.push({
              branchName: branchKey === '__null__' ? null : branchKey,
              sessions: [],
              sessionCount: 0,
              maxActivityAt: 0,
            })
          }
        }
      }

      return { ...project, branches }
    })

    // Add closed-only projects (no active sessions at all)
    for (const [projectKey, branchMap] of byProject) {
      if (coveredProjects.has(projectKey)) continue
      const branches = Array.from(branchMap.entries()).map(([branchKey]) => ({
        branchName: branchKey === '__null__' ? null : branchKey,
        sessions: [] as LiveSession[],
        sessionCount: 0,
        maxActivityAt: 0,
      }))
      for (const [branchKey] of branchMap) {
        coveredKeys.add(`${projectKey}::${branchKey}`)
      }
      merged.push({
        projectName: projectKey,
        projectPath: '',
        branches,
        totalSessionCount: 0,
        totalCostUsd: 0,
        maxActivityAt: 0,
      })
    }

    return { mergedProjects: merged, closedByKey: byKey }
  }, [projectGroups, recentlyClosed])

  return (
    <>
      <ColumnHeaders
        needsYouCount={needsYouSessions.length}
        autonomousCount={autonomousCount}
        needsYouSessions={needsYouSessions}
        closedCount={recentlyClosed.length}
        showClosed={showClosed}
        onDismissAll={onDismissAll}
      />
      <div className="flex-1 min-h-0 overflow-y-auto">
        {mergedProjects.map((project) => {
          const projKey = projectCollapseKey(project.projectName)
          const projCollapsed = isCollapsed(projKey)

          return (
            <div key={project.projectName} className="mb-2">
              <ProjectHeader
                projectName={project.projectName}
                projectPath={project.projectPath}
                totalCostUsd={project.totalCostUsd}
                sessionCount={project.totalSessionCount}
                isCollapsed={projCollapsed}
                onToggle={() => toggleCollapse(projKey)}
              />

              {!projCollapsed &&
                project.branches.map((branch) => {
                  const brKey = branchCollapseKey(project.projectName, branch.branchName)
                  const brCollapsed = isCollapsed(brKey)
                  const closedKey = `${project.projectName}::${branch.branchName ?? '__null__'}`
                  const branchClosed = closedByKey.get(closedKey) ?? []

                  // Skip branches with no active sessions and no closed sessions
                  if (branch.sessions.length === 0 && branchClosed.length === 0) return null

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
                          closedSessions={branchClosed}
                          selectedId={selectedId}
                          onSelect={onSelect}
                          stalledSessions={stalledSessions}
                          currentTime={currentTime}
                          onCardClick={onCardClick}
                          hideProjectBranch={true}
                          showClosed={showClosed}
                          onDismiss={onDismiss}
                        />
                      )}
                    </div>
                  )
                })}
            </div>
          )
        })}

        {mergedProjects.length === 0 && (
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
  projectGroups,
  isCollapsed,
  toggleCollapse,
  recentlyClosed = [],
  onDismiss,
  onDismissAll,
  showClosed = true,
}: KanbanViewProps) {
  return (
    <div className="flex flex-col h-full min-h-0 pb-4">
      {groupBy === 'none' ? (
        <FlatKanban
          sessions={sessions}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
          recentlyClosed={recentlyClosed}
          showClosed={showClosed}
          onDismiss={onDismiss}
          onDismissAll={onDismissAll}
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
          recentlyClosed={recentlyClosed}
          showClosed={showClosed}
          onDismiss={onDismiss}
          onDismissAll={onDismissAll}
        />
      )}
    </div>
  )
}
