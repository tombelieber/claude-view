import { useState, useRef, useCallback, useEffect, useMemo } from 'react'
import { Link, useLocation, useSearchParams } from 'react-router-dom'
import { ChevronRight, Folder, FolderOpen, Clock, Home, GitBranch, AlertCircle, List, FolderTree, ChevronsUpDown, ChevronsDownUp, BarChart3, X, ArrowRight, Server, Lightbulb, Monitor } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useProjectBranches } from '../hooks/use-branches'
import { cn } from '../lib/utils'
import { NO_BRANCH } from '../lib/constants'
import { buildFlatList, buildProjectTree, collectGroupNames, type ProjectTreeNode } from '../utils/build-project-tree'
import { useRecentSessions } from '../hooks/use-recent-sessions'
import { buildSessionUrl } from '../lib/url-utils'
import { getSessionTitle } from '../utils/get-session-title'

interface SidebarProps {
  projects: ProjectSummary[]
}

type ProjectViewMode = 'list' | 'tree'

export function Sidebar({ projects }: SidebarProps) {
  const location = useLocation()

  const [searchParams, setSearchParams] = useSearchParams()
  const selectedProjectId = searchParams.get("project")

  const [viewMode, setViewMode] = useState<ProjectViewMode>('list')
  const [expandedProjects, setExpandedProjects] = useState<Set<string>>(new Set())
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set())
  const [focusedIndex, setFocusedIndex] = useState<number>(-1)
  const itemRefs = useRef<(HTMLDivElement | null)[]>([])

  // Build tree structure based on view mode
  const treeNodes = useMemo(() => {
    return viewMode === 'tree' ? buildProjectTree(projects) : buildFlatList(projects)
  }, [projects, viewMode])

  // Auto-expand all groups when switching to tree mode
  const prevViewModeRef = useRef(viewMode)
  useEffect(() => {
    if (viewMode === 'tree' && prevViewModeRef.current !== 'tree') {
      setExpandedGroups(new Set(collectGroupNames(treeNodes)))
    }
    prevViewModeRef.current = viewMode
  }, [viewMode, treeNodes])

  // Auto-expand selected project from URL params (e.g. bookmarked URL with ?project=foo)
  useEffect(() => {
    if (selectedProjectId) {
      setExpandedProjects((prev) => {
        if (prev.has(selectedProjectId)) return prev
        return new Set(prev).add(selectedProjectId)
      })
    }
  }, [selectedProjectId])

  // Flatten tree nodes for keyboard navigation
  const flattenedNodes = useMemo(() => {
    const result: ProjectTreeNode[] = []
    function traverse(nodes: ProjectTreeNode[]) {
      for (const node of nodes) {
        result.push(node)
        if (node.type === 'group' && node.children && expandedGroups.has(node.name)) {
          traverse(node.children)
        }
      }
    }
    traverse(treeNodes)
    return result
  }, [treeNodes, expandedGroups])

  // Sync refs array length with flattened nodes
  useEffect(() => {
    itemRefs.current = itemRefs.current.slice(0, flattenedNodes.length)
  }, [flattenedNodes.length])

  const toggleExpandProject = useCallback((projectName: string, e?: React.MouseEvent) => {
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

  const toggleExpandGroup = useCallback((groupName: string, e?: React.MouseEvent) => {
    e?.stopPropagation()
    setExpandedGroups((prev) => {
      const next = new Set(prev)
      if (next.has(groupName)) {
        next.delete(groupName)
      } else {
        next.add(groupName)
      }
      return next
    })
  }, [])

  const handleProjectClick = useCallback((node: ProjectTreeNode) => {
    if (node.type !== 'project') return

    const currentProject = searchParams.get("project")

    // Toggle project filter via URL query params (expand is handled separately by chevron)
    const newParams = new URLSearchParams(searchParams)
    if (currentProject === node.name) {
      // Deselect: clear project and branch
      newParams.delete("project")
      newParams.delete("branch")
    } else {
      // Select: set project, clear branch
      newParams.set("project", node.name)
      newParams.delete("branch")
    }
    setSearchParams(newParams)
  }, [searchParams, setSearchParams])

  const handleGroupClick = useCallback((node: ProjectTreeNode) => {
    if (node.type !== 'group') return
    toggleExpandGroup(node.name)
  }, [toggleExpandGroup])

  const handleExpandAll = useCallback(() => {
    setExpandedGroups(new Set(collectGroupNames(treeNodes)))
    setExpandedProjects(new Set(projects.map((p) => p.name)))
  }, [treeNodes, projects])

  const handleCollapseAll = useCallback(() => {
    setExpandedGroups(new Set())
    setExpandedProjects(new Set())
  }, [])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (flattenedNodes.length === 0) return

    const currentNode = flattenedNodes[focusedIndex]

    switch (e.key) {
      case 'ArrowDown': {
        e.preventDefault()
        const next = focusedIndex < flattenedNodes.length - 1 ? focusedIndex + 1 : 0
        setFocusedIndex(next)
        itemRefs.current[next]?.focus()
        break
      }
      case 'ArrowUp': {
        e.preventDefault()
        const prev = focusedIndex > 0 ? focusedIndex - 1 : flattenedNodes.length - 1
        setFocusedIndex(prev)
        itemRefs.current[prev]?.focus()
        break
      }
      case 'Enter': {
        e.preventDefault()
        if (focusedIndex >= 0 && focusedIndex < flattenedNodes.length) {
          const node = flattenedNodes[focusedIndex]
          if (node.type === 'project') {
            handleProjectClick(node)
          } else {
            handleGroupClick(node)
          }
        }
        break
      }
      case 'ArrowRight': {
        e.preventDefault()
        if (currentNode) {
          if (currentNode.type === 'project') {
            if (!expandedProjects.has(currentNode.name)) {
              setExpandedProjects((prev) => new Set(prev).add(currentNode.name))
            }
          } else if (currentNode.type === 'group') {
            if (!expandedGroups.has(currentNode.name)) {
              setExpandedGroups((prev) => new Set(prev).add(currentNode.name))
            }
          }
        }
        break
      }
      case 'ArrowLeft': {
        e.preventDefault()
        if (currentNode) {
          if (currentNode.type === 'project') {
            if (expandedProjects.has(currentNode.name)) {
              setExpandedProjects((prev) => {
                const next = new Set(prev)
                next.delete(currentNode.name)
                return next
              })
            }
          } else if (currentNode.type === 'group') {
            if (expandedGroups.has(currentNode.name)) {
              setExpandedGroups((prev) => {
                const next = new Set(prev)
                next.delete(currentNode.name)
                return next
              })
            }
          }
        }
        break
      }
    }
  }, [focusedIndex, flattenedNodes, expandedProjects, expandedGroups, handleProjectClick, handleGroupClick])

  // Render tree node recursively
  const renderTreeNode = useCallback((node: ProjectTreeNode, index: number) => {
    if (node.type === 'group') {
      const isExpanded = expandedGroups.has(node.name)
      const paddingLeft = node.depth * 12 + 8

      return (
        <div key={`group-${node.name}`}>
          <div
            ref={(el) => { itemRefs.current[index] = el }}
            role="treeitem"
            aria-expanded={isExpanded}
            tabIndex={focusedIndex === index ? 0 : -1}
            onClick={() => handleGroupClick(node)}
            onFocus={() => setFocusedIndex(index)}
            style={{ paddingLeft: `${paddingLeft}px` }}
            className={cn(
              'w-full flex items-center gap-1 py-1 pr-2 h-7 cursor-pointer select-none',
              'transition-colors duration-150',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 focus-visible:outline-none',
              'text-gray-700 dark:text-gray-300 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
            )}
          >
            {/* Chevron toggle */}
            <button
              type="button"
              tabIndex={-1}
              aria-label={isExpanded ? 'Collapse' : 'Expand'}
              onClick={(e) => toggleExpandGroup(node.name, e)}
              className={cn(
                'flex-shrink-0 p-0.5 rounded hover:bg-black/10 transition-transform',
                isExpanded && 'rotate-90'
              )}
            >
              <ChevronRight className="w-3.5 h-3.5" />
            </button>

            {/* Folder icon */}
            {isExpanded ? (
              <FolderOpen className="w-4 h-4 flex-shrink-0 text-gray-400 dark:text-gray-500" />
            ) : (
              <Folder className="w-4 h-4 flex-shrink-0 text-gray-400 dark:text-gray-500" />
            )}

            {/* Group name */}
            <span className="flex-1 truncate font-medium text-[13px] ml-1 text-gray-600 dark:text-gray-400">
              {node.displayName}
            </span>

            {/* Session count */}
            <span className="text-[11px] tabular-nums flex-shrink-0 text-gray-400 dark:text-gray-500">
              {node.sessionCount}
            </span>
          </div>
        </div>
      )
    } else {
      // Project node
      const isSelected = selectedProjectId === node.name
      const isExpanded = expandedProjects.has(node.name)
      const paddingLeft = node.depth * 12 + 8

      return (
        <div key={`project-${node.name}`}>
          <div
            ref={(el) => { itemRefs.current[index] = el }}
            role="treeitem"
            aria-selected={isSelected}
            aria-expanded={isExpanded}
            aria-current={isSelected ? 'page' : undefined}
            tabIndex={focusedIndex === index ? 0 : -1}
            onClick={() => handleProjectClick(node)}
            onFocus={() => setFocusedIndex(index)}
            style={{ paddingLeft: `${paddingLeft}px` }}
            className={cn(
              'w-full flex items-center gap-1 py-1 pr-2 h-7 cursor-pointer select-none',
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
              onClick={(e) => toggleExpandProject(node.name, e)}
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
              {node.displayName}
            </span>

            {/* Session count */}
            <span className={cn(
              'text-[11px] tabular-nums flex-shrink-0',
              isSelected ? 'text-blue-100' : 'text-gray-400 dark:text-gray-500'
            )}>
              {node.sessionCount}
            </span>
          </div>

          {/* Expanded content - Branch list */}
          {isExpanded && (
            <BranchList
              projectName={node.name}
            />
          )}
        </div>
      )
    }
  }, [
    selectedProjectId,
    expandedProjects,
    expandedGroups,
    focusedIndex,
    toggleExpandProject,
    toggleExpandGroup,
    handleProjectClick,
    handleGroupClick,
  ])

  return (
    <aside className="w-72 bg-gray-50/80 dark:bg-gray-900/80 border-r border-gray-200 dark:border-gray-700 flex flex-col overflow-hidden">
      {/* ─── Zone 1: Navigation Tabs ─── */}
      <nav className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 space-y-1" aria-label="Main navigation">
        {(() => {
          // Build preserved params string for nav links
          const preservedParams = new URLSearchParams()
          if (searchParams.get("project")) preservedParams.set("project", searchParams.get("project")!)
          if (searchParams.get("branch")) preservedParams.set("branch", searchParams.get("branch")!)
          const paramString = preservedParams.toString()

          return (
            <>
              <Link
                to={`/${paramString ? `?${paramString}` : ""}`}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname === '/'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <Home className="w-4 h-4" />
                <span className="font-medium">Fluency</span>
              </Link>
              <Link
                to={`/sessions${paramString ? `?${paramString}` : ""}`}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname.startsWith('/sessions')
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <Clock className="w-4 h-4" />
                <span className="font-medium">Sessions</span>
              </Link>
              <Link
                to={`/contributions${paramString ? `?${paramString}` : ""}`}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname === '/contributions'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <BarChart3 className="w-4 h-4" />
                <span className="font-medium">Contributions</span>
              </Link>
              <Link
                to={`/insights${paramString ? `?${paramString}` : ""}`}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname === '/insights'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <Lightbulb className="w-4 h-4" />
                <span className="font-medium">Insights</span>
              </Link>
              <Link
                to={`/system${paramString ? `?${paramString}` : ""}`}
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname === '/system'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <Server className="w-4 h-4" />
                <span className="font-medium">System</span>
              </Link>
              <Link
                to="/mission-control"
                className={cn(
                  'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  location.pathname === '/mission-control'
                    ? 'bg-blue-500 text-white'
                    : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
                )}
              >
                <Monitor className="w-4 h-4" />
                <span className="font-medium">Mission Control</span>
              </Link>
            </>
          )
        })()}
      </nav>

      {/* ─── Zone 2: Scope Panel ─── */}
      <div className="flex flex-col min-h-0 flex-1">
        {/* Scope header with clear button */}
        <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center justify-between mb-2">
            <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
              Scope
            </span>
            {selectedProjectId && (
              <button
                type="button"
                onClick={() => {
                  const newParams = new URLSearchParams(searchParams)
                  newParams.delete('project')
                  newParams.delete('branch')
                  setSearchParams(newParams)
                }}
                className="text-[10px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors flex items-center gap-0.5 rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400"
                aria-label="Clear scope"
              >
                <X className="w-3 h-3" />
                Clear
              </button>
            )}
          </div>

          {/* View mode toggle + expand/collapse controls */}
          <div className="flex items-center gap-2">
            <div className="flex items-center gap-0.5 p-0.5 bg-gray-100 dark:bg-gray-800 rounded-md">
              <button
                type="button"
                onClick={() => setViewMode('list')}
                className={cn(
                  'flex-1 inline-flex items-center justify-center gap-1.5 px-2 py-1.5 text-xs font-medium rounded transition-all',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  viewMode === 'list'
                    ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
                )}
                aria-label="List view"
                aria-pressed={viewMode === 'list'}
              >
                <List className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                onClick={() => setViewMode('tree')}
                className={cn(
                  'flex-1 inline-flex items-center justify-center gap-1.5 px-2 py-1.5 text-xs font-medium rounded transition-all',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                  viewMode === 'tree'
                    ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
                )}
                aria-label="Tree view"
                aria-pressed={viewMode === 'tree'}
              >
                <FolderTree className="w-3.5 h-3.5" />
              </button>
            </div>
            <div className="flex items-center gap-0.5 ml-auto">
              <button
                type="button"
                onClick={handleExpandAll}
                title="Expand All"
                className={cn(
                  'p-1 rounded transition-colors',
                  'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
                  'hover:bg-gray-200/70 dark:hover:bg-gray-700/70',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400'
                )}
              >
                <ChevronsUpDown className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                onClick={handleCollapseAll}
                title="Collapse All"
                className={cn(
                  'p-1 rounded transition-colors',
                  'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
                  'hover:bg-gray-200/70 dark:hover:bg-gray-700/70',
                  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400'
                )}
              >
                <ChevronsDownUp className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </div>

        {/* Project tree (scrollable) */}
        <div
          className="flex-1 overflow-y-auto py-1"
          role="tree"
          aria-label="Projects"
          onKeyDown={handleKeyDown}
        >
          {flattenedNodes.map((node, i) => renderTreeNode(node, i))}
        </div>
      </div>

      {/* ─── Zone 3: Quick Jump (only when scoped) ─── */}
      {selectedProjectId && (
        <QuickJumpZone project={selectedProjectId} branch={searchParams.get('branch')} />
      )}
    </aside>
  )
}

interface BranchListProps {
  projectName: string
}

function BranchList({ projectName }: BranchListProps) {
  const [searchParams, setSearchParams] = useSearchParams()
  const branchParam = searchParams.get('branch') || ''
  const selectedProject = searchParams.get('project')
  const activeBranches = useMemo(
    () => {
      // Only show active branches if this project is the selected one
      if (selectedProject !== projectName) return new Set<string>()
      return new Set(branchParam ? [branchParam] : [])
    },
    [branchParam, selectedProject, projectName]
  )
  const { data, isLoading, error, refetch } = useProjectBranches(projectName)

  const handleBranchClick = useCallback((branch: string | null) => {
    const newParams = new URLSearchParams(searchParams)
    const branchKey = branch ?? NO_BRANCH // Encode null → ~ (git-invalid sentinel)

    if (!activeBranches.has(branchKey)) {
      // Select branch — also ensure parent project is set
      newParams.set('branch', branchKey)
      newParams.set('project', projectName)
    } else {
      // Deselect branch (toggle off)
      newParams.delete('branch')
    }
    setSearchParams(newParams)
  }, [searchParams, setSearchParams, activeBranches, projectName])

  if (isLoading) {
    return (
      <div className="pl-10 pr-3 py-2 space-y-1">
        {[1, 2, 3].map((i) => (
          <div
            key={i}
            className="h-5 bg-gray-200 dark:bg-gray-700 rounded animate-pulse"
            style={{ width: `${60 + i * 10}%` }}
          />
        ))}
      </div>
    )
  }

  if (error) {
    return (
      <div className="pl-10 pr-3 py-2">
        <div className="flex items-center gap-2 text-[11px] text-red-600 dark:text-red-400">
          <AlertCircle className="w-3.5 h-3.5 flex-shrink-0" />
          <span>Failed to load branches</span>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          className="mt-1 text-[11px] text-blue-600 dark:text-blue-400 hover:underline rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!data || data.branches.length === 0) {
    return (
      <div className="pl-10 pr-3 py-1">
        <span className="text-[11px] text-gray-400 dark:text-gray-500">
          No branches
        </span>
      </div>
    )
  }

  return (
    <div className="pl-8 pr-2 py-1">
      {data.branches.map((branchItem) => {
        const displayName = branchItem.branch || '(no branch)'
        const isNoBranch = !branchItem.branch
        const effectiveBranch = branchItem.branch ?? NO_BRANCH
        const isActive = activeBranches.has(effectiveBranch)

        return (
          <button
            key={branchItem.branch || '__no_branch__'}
            type="button"
            onClick={() => handleBranchClick(branchItem.branch)}
            title={branchItem.branch || 'Sessions without a git branch'}
            className={cn(
              'w-full flex items-center gap-1.5 px-2 py-1 h-6 rounded',
              'transition-colors duration-150 cursor-pointer',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 focus-visible:outline-none',
              isActive
                ? 'bg-blue-100 dark:bg-blue-900/40'
                : 'hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
            )}
          >
            <GitBranch className={cn(
              'w-3 h-3 flex-shrink-0',
              isActive ? 'text-blue-500 dark:text-blue-400' : 'text-gray-400 dark:text-gray-500'
            )} />
            <span
              className={cn(
                'flex-1 truncate text-[11px] text-left',
                isNoBranch && 'italic',
                isActive
                  ? 'text-blue-700 dark:text-blue-300 font-medium'
                  : 'text-gray-600 dark:text-gray-400'
              )}
            >
              {displayName}
            </span>
            <span className={cn(
              'text-[10px] tabular-nums flex-shrink-0',
              isActive ? 'text-blue-500 dark:text-blue-400' : 'text-gray-400 dark:text-gray-500'
            )}>
              {branchItem.count}
            </span>
          </button>
        )
      })}
    </div>
  )
}

function QuickJumpZone({ project, branch }: { project: string; branch: string | null }) {
  const [searchParams] = useSearchParams()
  const { data: sessions, isLoading } = useRecentSessions(project, branch)

  if (isLoading) {
    return (
      <div className="border-t border-gray-200 dark:border-gray-700 px-3 py-2">
        <div className="h-4 w-16 bg-gray-200 dark:bg-gray-700 rounded animate-pulse mb-2" />
        {[1, 2, 3].map(i => (
          <div key={i} className="h-6 bg-gray-200 dark:bg-gray-700 rounded animate-pulse mb-1.5" style={{ width: `${60 + i * 10}%` }} />
        ))}
      </div>
    )
  }

  if (!sessions || sessions.length === 0) return null

  return (
    <nav aria-label="Recent sessions" className="border-t border-gray-200 dark:border-gray-700 px-3 py-2">
      <div className="flex items-center justify-between mb-1.5">
        <span className="text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
          Recent
        </span>
        <Link
          to={`/sessions?project=${encodeURIComponent(project)}${branch ? `&branch=${encodeURIComponent(branch)}` : ''}`}
          className="text-[10px] text-gray-400 hover:text-blue-500 transition-colors flex items-center gap-0.5 rounded focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400"
        >
          All <ArrowRight className="w-2.5 h-2.5" />
        </Link>
      </div>
      <div className="space-y-0.5">
        {sessions.map(session => (
          <Link
            key={session.id}
            to={buildSessionUrl(session.id, searchParams)}
            className={cn(
              'flex items-center gap-2 px-2 py-1 h-6 rounded text-[11px] transition-colors',
              'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70',
              'focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 focus-visible:outline-none'
            )}
            title={session.preview}
          >
            <Clock className="w-3 h-3 flex-shrink-0 text-gray-400 dark:text-gray-500" />
            <span className="truncate flex-1">{getSessionTitle(session.preview, session.summary)}</span>
            <span className="text-[10px] text-gray-400 dark:text-gray-500 tabular-nums flex-shrink-0">
              {formatRelativeTimeShort(session.modifiedAt)}
            </span>
          </Link>
        ))}
      </div>
    </nav>
  )
}

function formatRelativeTimeShort(timestamp: number): string {
  const diff = Date.now() / 1000 - timestamp
  if (diff < 60) return 'now'
  if (diff < 3600) return `${Math.floor(diff / 60)}m`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`
  return `${Math.floor(diff / 86400)}d`
}
