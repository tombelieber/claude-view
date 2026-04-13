import { Archive, ChevronRight, Folder } from 'lucide-react'
import { useState } from 'react'
import type { ProjectSummary } from '../../hooks/use-projects'
import type { ProjectTreeNode } from '../../utils/build-project-tree'
import { cn } from '../../lib/utils'

interface ArchivedProjectsSectionProps {
  projects: ProjectSummary[]
  onProjectClick: (node: ProjectTreeNode) => void
  selectedProjectId: string | null
}

export function ArchivedProjectsSection({
  projects,
  onProjectClick,
  selectedProjectId,
}: ArchivedProjectsSectionProps) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="mt-1 border-t border-gray-200 dark:border-gray-700 pt-1">
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className={cn(
          'w-full flex items-center gap-1.5 px-3 py-1.5 h-7 text-xs select-none',
          'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-400',
          'hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors',
        )}
      >
        <ChevronRight
          className={cn('w-3 h-3 transition-transform flex-shrink-0', expanded && 'rotate-90')}
        />
        <Archive className="w-3.5 h-3.5 flex-shrink-0" />
        <span className="font-medium">Archived</span>
        <span className="ml-auto tabular-nums text-gray-400 dark:text-gray-600">
          {projects.length}
        </span>
      </button>

      {expanded && (
        <div className="py-0.5">
          {[...projects]
            .sort((a, b) => (b.sessionCount ?? 0) - (a.sessionCount ?? 0))
            .map((project) => {
              const isSelected = selectedProjectId === project.name
              return (
                <div
                  key={project.name}
                  role="button"
                  aria-selected={isSelected}
                  onClick={() =>
                    onProjectClick({
                      type: 'project',
                      name: project.name,
                      displayName: project.displayName,
                      path: project.path,
                      sessionCount: project.sessionCount,
                      depth: 0,
                      isArchived: true,
                    })
                  }
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault()
                      onProjectClick({
                        type: 'project',
                        name: project.name,
                        displayName: project.displayName,
                        path: project.path,
                        sessionCount: project.sessionCount,
                        depth: 0,
                        isArchived: true,
                      })
                    }
                  }}
                  tabIndex={0}
                  className={cn(
                    'w-full flex items-center gap-1 py-1 pr-2 pl-10 h-7 cursor-pointer select-none',
                    'transition-colors duration-150',
                    isSelected
                      ? 'bg-blue-500 text-white'
                      : 'text-gray-500 dark:text-gray-500 hover:bg-gray-200/70 dark:hover:bg-gray-800/70',
                  )}
                >
                  <Folder
                    className={cn(
                      'w-4 h-4 flex-shrink-0',
                      isSelected ? 'text-white' : 'text-gray-400 dark:text-gray-600',
                    )}
                  />
                  <span className="flex-1 truncate text-xs ml-1">{project.displayName}</span>
                  <span
                    className={cn(
                      'text-xs tabular-nums flex-shrink-0',
                      isSelected ? 'text-blue-100' : 'text-gray-400 dark:text-gray-600',
                    )}
                  >
                    {project.sessionCount}
                  </span>
                </div>
              )
            })}
        </div>
      )}
    </div>
  )
}
