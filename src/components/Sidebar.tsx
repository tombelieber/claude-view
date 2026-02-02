import { useState, useRef, useCallback, useEffect } from 'react'
import { Link, useParams, useLocation, useNavigate } from 'react-router-dom'
import { ChevronRight, Folder, FolderOpen, Clock } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface SidebarProps {
  projects: ProjectSummary[]
}

export function Sidebar({ projects }: SidebarProps) {
  const params = useParams()
  const location = useLocation()
  const navigate = useNavigate()

  const selectedProjectId = params.projectId ? decodeURIComponent(params.projectId) : null

  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set())
  const [focusedIndex, setFocusedIndex] = useState<number>(-1)
  const itemRefs = useRef<(HTMLDivElement | null)[]>([])

  // Sync refs array length with projects
  useEffect(() => {
    itemRefs.current = itemRefs.current.slice(0, projects.length)
  }, [projects.length])

  const toggleExpand = useCallback((projectName: string, e?: React.MouseEvent) => {
    e?.stopPropagation()
    setExpandedProjects((prev) => {
      const next = new Set(prev)
      if (next.has(projectName)) {
        next.delete(projectName)
      } else {
        next.add(projectName)
      }
      return next
    })
  }, [])

  const handleRowClick = useCallback((project: ProjectSummary) => {
    // Navigate and expand
    if (!expandedProjects.has(project.name)) {
      setExpandedProjects((prev) => new Set(prev).add(project.name))
    }
    navigate(`/project/${encodeURIComponent(project.name)}`)
  }, [expandedProjects, navigate])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (projects.length === 0) return

    switch (e.key) {
      case 'ArrowDown': {
        e.preventDefault()
        const next = focusedIndex < projects.length - 1 ? focusedIndex + 1 : 0
        setFocusedIndex(next)
        itemRefs.current[next]?.focus()
        break
      }
      case 'ArrowUp': {
        e.preventDefault()
        const prev = focusedIndex > 0 ? focusedIndex - 1 : projects.length - 1
        setFocusedIndex(prev)
        itemRefs.current[prev]?.focus()
        break
      }
      case 'Enter': {
        e.preventDefault()
        if (focusedIndex >= 0 && focusedIndex < projects.length) {
          handleRowClick(projects[focusedIndex])
        }
        break
      }
      case 'ArrowRight': {
        e.preventDefault()
        if (focusedIndex >= 0 && focusedIndex < projects.length) {
          const name = projects[focusedIndex].name
          if (!expandedProjects.has(name)) {
            setExpandedProjects((prev) => new Set(prev).add(name))
          }
        }
        break
      }
      case 'ArrowLeft': {
        e.preventDefault()
        if (focusedIndex >= 0 && focusedIndex < projects.length) {
          const name = projects[focusedIndex].name
          if (expandedProjects.has(name)) {
            setExpandedProjects((prev) => {
              const next = new Set(prev)
              next.delete(name)
              return next
            })
          }
        }
        break
      }
    }
  }, [focusedIndex, projects, expandedProjects, handleRowClick])

  return (
    <aside className="w-72 bg-gray-50/80 dark:bg-gray-900/80 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden">
      {/* Nav Links */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
        <Link
          to="/history"
          className={cn(
            'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            location.pathname === '/history'
              ? 'bg-blue-500 text-white'
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
          )}
        >
          <Clock className="w-4 h-4" />
          <span className="font-medium">History</span>
        </Link>
      </div>

      {/* Project Tree */}
      <div
        className="flex-1 overflow-y-auto py-1"
        role="tree"
        aria-label="Projects"
        onKeyDown={handleKeyDown}
      >
        {projects.map((project, i) => {
          const isSelected = selectedProjectId === project.name
          const isExpanded = expandedProjects.has(project.name)

          return (
            <div key={project.name}>
              <div
                ref={(el) => { itemRefs.current[i] = el }}
                role="treeitem"
                aria-selected={isSelected}
                aria-expanded={isExpanded}
                aria-current={isSelected ? 'page' : undefined}
                tabIndex={focusedIndex === i ? 0 : -1}
                onClick={() => handleRowClick(project)}
                onFocus={() => setFocusedIndex(i)}
                className={cn(
                  'w-full flex items-center gap-1 px-2 py-1 h-7 cursor-pointer select-none',
                  'transition-colors duration-150',
                  'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 focus-visible:outline-none',
                  isSelected
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-700 dark:text-gray-300 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                {/* Chevron toggle */}
                <button
                  type="button"
                  tabIndex={-1}
                  aria-label={isExpanded ? 'Collapse' : 'Expand'}
                  onClick={(e) => toggleExpand(project.name, e)}
                  className={cn(
                    'flex-shrink-0 p-0.5 rounded hover:bg-black/10 transition-transform',
                    isExpanded && 'rotate-90'
                  )}
                >
                  <ChevronRight className="w-3.5 h-3.5" />
                </button>

                {/* Folder icon */}
                {isExpanded ? (
                  <FolderOpen className={cn(
                    'w-4 h-4 flex-shrink-0',
                    isSelected ? 'text-white' : 'text-blue-400'
                  )} />
                ) : (
                  <Folder className={cn(
                    'w-4 h-4 flex-shrink-0',
                    isSelected ? 'text-white' : 'text-blue-400'
                  )} />
                )}

                {/* Project name */}
                <span className="flex-1 truncate font-medium text-[13px] ml-1">
                  {project.displayName}
                </span>

                {/* Session count */}
                <span className={cn(
                  'text-[11px] tabular-nums flex-shrink-0',
                  isSelected ? 'text-blue-100' : 'text-gray-400 dark:text-gray-500'
                )}>
                  {project.sessionCount}
                </span>
              </div>

              {/* Expanded content */}
              {isExpanded && (
                <div className="pl-10 pr-3 py-1">
                  <span className={cn(
                    'text-[11px]',
                    isSelected ? 'text-gray-500' : 'text-gray-400 dark:text-gray-500'
                  )}>
                    {project.sessionCount} session{project.sessionCount !== 1 ? 's' : ''}
                  </span>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </aside>
  )
}
