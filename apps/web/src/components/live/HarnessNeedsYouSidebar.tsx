import { Check, ChevronDown, ChevronRight, FolderOpen, GitBranch } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { cn } from '../../lib/utils'
import { SessionCard } from './SessionCard'
import type { LiveSession } from './use-live-sessions'

interface ProjectBranchGroup {
  project: string
  branch: string | null
  sessions: LiveSession[]
}

interface HarnessNeedsYouSidebarProps {
  sessions: LiveSession[]
  collapsed: boolean
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
}

export function HarnessNeedsYouSidebar({
  sessions,
  collapsed,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: HarnessNeedsYouSidebarProps) {
  if (collapsed) {
    return (
      <div className="h-full flex items-start justify-center pt-4">
        <div className="w-8 h-8 rounded-full bg-green-100 dark:bg-green-900/30 flex items-center justify-center">
          <Check className="w-4 h-4 text-green-600 dark:text-green-400" />
        </div>
      </div>
    )
  }

  return (
    <ExpandedSidebar
      sessions={sessions}
      selectedId={selectedId}
      onSelect={onSelect}
      stalledSessions={stalledSessions}
      currentTime={currentTime}
      onCardClick={onCardClick}
    />
  )
}

function ExpandedSidebar({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: Omit<HarnessNeedsYouSidebarProps, 'collapsed'>) {
  const urgentCount = useMemo(
    () =>
      sessions.filter(
        (s) => s.agentState.state === 'awaiting_input' || s.agentState.state === 'needs_permission',
      ).length,
    [sessions],
  )

  return (
    <div className="flex flex-col h-full min-h-0">
      {/* Header — amber stripe matching Board view */}
      <div className="shrink-0 mb-3">
        <div className="relative bg-gray-50/50 dark:bg-gray-900/50 rounded-lg border border-gray-200 dark:border-gray-800">
          <div className="h-0.5 rounded-t-lg bg-amber-500" />
          <div className="px-3 py-2 flex items-center gap-1.5">
            <span className="text-sm font-medium text-gray-700 dark:text-gray-300">Needs You</span>
            <span className="text-xs text-gray-400 dark:text-gray-500">({sessions.length})</span>
            {urgentCount > 0 && (
              <span className="ml-1.5 text-xs text-amber-500 font-normal">
                {urgentCount} urgent
              </span>
            )}
          </div>
        </div>
      </div>

      {/* Scrollable card list grouped by project/branch */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <GroupedSidebarCards
          sessions={sessions}
          selectedId={selectedId}
          onSelect={onSelect}
          stalledSessions={stalledSessions}
          currentTime={currentTime}
          onCardClick={onCardClick}
        />
      </div>
    </div>
  )
}

function GroupedSidebarCards({
  sessions,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
}: Omit<HarnessNeedsYouSidebarProps, 'collapsed'>) {
  const groups = useMemo(() => {
    const map = new Map<string, ProjectBranchGroup>()
    for (const s of sessions) {
      const project = s.projectDisplayName || s.project || 'Unknown'
      const branch = s.effectiveBranch ?? null
      const key = `${project}::${branch ?? '__null__'}`
      let group = map.get(key)
      if (!group) {
        group = { project, branch, sessions: [] }
        map.set(key, group)
      }
      group.sessions.push(s)
    }
    return Array.from(map.values())
  }, [sessions])

  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set())
  const toggleGroup = useCallback((key: string) => {
    setCollapsedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }, [])

  return (
    <div className="space-y-3">
      {groups.map((group) => {
        const groupKey = `${group.project}::${group.branch}`
        const isGroupCollapsed = collapsedGroups.has(groupKey)
        const Chevron = isGroupCollapsed ? ChevronRight : ChevronDown

        return (
          <div key={groupKey}>
            {/* Collapsible project/branch header */}
            <button
              type="button"
              onClick={() => toggleGroup(groupKey)}
              className="flex items-center gap-1.5 px-1 mb-1.5 w-full text-left cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 rounded py-0.5 transition-colors"
            >
              <Chevron className="w-3 h-3 shrink-0 text-gray-400" />
              <FolderOpen className="w-3 h-3 shrink-0 text-amber-500 dark:text-amber-400" />
              <span className="text-xs font-medium text-gray-600 dark:text-gray-400 truncate">
                {group.project}
              </span>
              {group.branch && (
                <>
                  <span className="text-gray-300 dark:text-gray-600">/</span>
                  <GitBranch className="w-3 h-3 shrink-0 text-violet-500 dark:text-violet-400" />
                  <span className="text-xs text-gray-500 dark:text-gray-500 truncate">
                    {group.branch}
                  </span>
                </>
              )}
              <span className="text-xs text-gray-400 dark:text-gray-500 ml-auto shrink-0">
                {group.sessions.length}
              </span>
            </button>
            {/* Cards — hide project/branch on card since header shows it */}
            {!isGroupCollapsed && (
              <div className="space-y-2">
                {group.sessions.map((session) => (
                  <button
                    type="button"
                    key={session.id}
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
                      session.cacheStatus !== 'warm' && 'opacity-70',
                    )}
                  >
                    <SessionCard
                      session={session}
                      stalledSessions={stalledSessions}
                      currentTime={currentTime}
                      onClickOverride={onCardClick ? () => onCardClick(session.id) : undefined}
                      showStateBadge
                      hideProjectBranch
                    />
                  </button>
                ))}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
