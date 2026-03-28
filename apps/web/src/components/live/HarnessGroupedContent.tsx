import { useMemo } from 'react'
import { BranchHeader, ProjectHeader } from './KanbanSwimLaneHeader'
import { HarnessPhaseRow } from './HarnessPhaseRow'
import {
  type PhaseColumn,
  getSessionPhase,
  isDesignPhase,
  sortNeedsYouFirst,
  splitByPhase,
} from './harness-phase-groups'
import type { ProjectGroup } from './use-kanban-grouping'
import { branchCollapseKey, projectCollapseKey } from './use-kanban-grouping'
import type { LiveSession } from './use-live-sessions'

interface FilteredBranch {
  branchName: string | null
  sessions: LiveSession[]
  sessionCount: number
}

interface GroupedContentProps {
  phases: readonly PhaseColumn[]
  projectGroups: ProjectGroup[]
  isDesign: boolean
  selectedId: string | null
  onSelect: (id: string) => void
  stalledSessions?: Set<string>
  currentTime: number
  onCardClick?: (sessionId: string) => void
  isCollapsed: (key: string) => boolean
  toggleCollapse: (key: string) => void
}

export function HarnessGroupedContent({
  phases,
  projectGroups,
  isDesign,
  selectedId,
  onSelect,
  stalledSessions,
  currentTime,
  onCardClick,
  isCollapsed,
  toggleCollapse,
}: GroupedContentProps) {
  // Filter project groups to only those with sessions in this phase group
  const relevantProjects = useMemo(() => {
    const result: { project: ProjectGroup; filteredBranches: FilteredBranch[] }[] = []
    for (const project of projectGroups) {
      const filteredBranches: FilteredBranch[] = []
      for (const branch of project.branches) {
        const matching = branch.sessions.filter(
          (s) => isDesignPhase(getSessionPhase(s)) === isDesign,
        )
        if (matching.length > 0) {
          filteredBranches.push({
            branchName: branch.branchName,
            sessions: sortNeedsYouFirst(matching),
            sessionCount: matching.length,
          })
        }
      }
      if (filteredBranches.length > 0) {
        result.push({ project, filteredBranches })
      }
    }
    return result
  }, [projectGroups, isDesign])

  if (relevantProjects.length === 0) {
    return (
      <div className="text-center text-gray-300 dark:text-gray-700 py-4 text-xs">{'\u2014'}</div>
    )
  }

  return (
    <div className="space-y-1">
      {relevantProjects.map(({ project, filteredBranches }) => {
        const pKey = projectCollapseKey(project.projectName)
        const pCollapsed = isCollapsed(pKey)
        const pCount = filteredBranches.reduce((sum, b) => sum + b.sessionCount, 0)

        return (
          <div key={project.projectName}>
            <ProjectHeader
              projectName={project.projectName}
              projectPath={project.projectPath}
              totalCostUsd={project.totalCostUsd}
              sessionCount={pCount}
              isCollapsed={pCollapsed}
              onToggle={() => toggleCollapse(pKey)}
            />
            {!pCollapsed &&
              filteredBranches.map((branch) => {
                const bKey = branchCollapseKey(project.projectName, branch.branchName)
                const bCollapsed = isCollapsed(bKey)
                const byPhase = splitByPhase(branch.sessions, phases)

                return (
                  <div key={branch.branchName ?? '__null__'}>
                    <BranchHeader
                      branchName={branch.branchName}
                      sessionCount={branch.sessionCount}
                      isCollapsed={bCollapsed}
                      onToggle={() => toggleCollapse(bKey)}
                    />
                    {!bCollapsed && (
                      <HarnessPhaseRow
                        phases={phases}
                        byPhase={byPhase}
                        selectedId={selectedId}
                        onSelect={onSelect}
                        stalledSessions={stalledSessions}
                        currentTime={currentTime}
                        onCardClick={onCardClick}
                        hideProjectBranch
                      />
                    )}
                  </div>
                )
              })}
          </div>
        )
      })}
    </div>
  )
}
