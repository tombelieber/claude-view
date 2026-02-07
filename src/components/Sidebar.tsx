import { useState, useRef, useCallback, useEffect, useMemo } from 'react'
import { Link, useParams, useLocation, useNavigate, useSearchParams } from 'react-router-dom'
import { ChevronRight, Folder, FolderOpen, Clock, GitBranch, AlertCircle, List, FolderTree, ChevronsUpDown, ChevronsDownUp } from 'lucide-react'
import type { ProjectSummary } from '../hooks/use-projects'
import { useProjectBranches } from '../hooks/use-branches'
import { cn } from '../lib/utils'
import { buildFlatList, buildProjectTree, collectGroupNames, type ProjectTreeNode } from '../utils/build-project-tree'

interface SidebarProps {
  projects: ProjectSummary[]
}

type ProjectViewMode = 'list' | 'tree'

export function Sidebar({ projects }: SidebarProps) {
  const params = useParams()
  const location = useLocation()
  const navigate = useNavigate()

  const selectedProjectId = params.projectId ? decodeURIComponent(params.projectId) : null

  const [searchParams] = useSearchParams()
  // On contributions page, the filtered project comes from URL search params
  const contributionsProjectId = location.pathname === '/contributions'
    ? searchParams.get('projectId')
    : null

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

    // Toggle expand/collapse
    setExpandedProjects((prev) => {
      const next = new Set(prev)
      if (next.has(node.name)) {
        next.delete(node.name)
      } else {
        next.add(node.name)
      }
      return next
    })

    // Context-aware navigation
    if (location.pathname === '/contributions') {
      // Stay on contributions, set projectId URL param
      const params = new URLSearchParams(window.location.search)
      const current = params.get('projectId')
      if (current === node.name) {
        params.delete('projectId') // Toggle off = back to All Projects
      } else {
        params.set('projectId', node.name)
      }
      navigate(`/contributions?${params}`)
    } else {
      // Default: navigate to project detail page
      navigate(`/project/${encodeURIComponent(node.name)}`)
    }
  }, [navigate, location.pathname])

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
      const isSelected = selectedProjectId === node.name || contributionsProjectId === node.name
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
              isSelected={isSelected}
            />
          )}
        </div>
      )
    }
  }, [
    selectedProjectId,
    contributionsProjectId,
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
      {/* Nav Links */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 space-y-1">
        <Link
          to="/contributions"
          className={cn(
            'flex items-center gap-2 px-2 py-1.5 rounded-md text-sm transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            location.pathname === '/contributions'
              ? 'bg-blue-500 text-white'
              : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200/70 dark:hover:bg-gray-800/70'
          )}
        >
          <GitBranch className="w-4 h-4" />
          <span className="font-medium">Contributions</span>
        </Link>
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

      {/* View Mode Toggle + Expand/Collapse */}
      <div className="px-3 py-2 border-b border-gray-200 dark:border-gray-700 flex items-center gap-2">
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

      {/* Project Tree */}
      <div
        className="flex-1 overflow-y-auto py-1"
        role="tree"
        aria-label="Projects"
        onKeyDown={handleKeyDown}
      >
        {flattenedNodes.map((node, i) => renderTreeNode(node, i))}
      </div>
    </aside>
  )
}

interface BranchListProps {
  projectName: string
  isSelected: boolean
}

function BranchList({ projectName, isSelected }: BranchListProps) {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const activeBranch = searchParams.get('branches') || null
  const { data, isLoading, error, refetch } = useProjectBranches(projectName)

  const handleBranchClick = useCallback((branch: string | null) => {
    // Preserve existing URL params (filters, sort, groupBy, etc.)
    const params = new URLSearchParams(window.location.search)
    if (branch) {
      params.set('branches', branch)
    } else {
      params.delete('branches')
    }
    navigate(`/project/${encodeURIComponent(projectName)}?${params}`)
  }, [projectName, navigate])

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
          className="mt-1 text-[11px] text-blue-600 dark:text-blue-400 hover:underline"
        >
          Retry
        </button>
      </div>
    )
  }

  if (!data || data.branches.length === 0) {
    return (
      <div className="pl-10 pr-3 py-1">
        <span className={cn(
          'text-[11px]',
          isSelected ? 'text-gray-500' : 'text-gray-400 dark:text-gray-500'
        )}>
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
        const isActive = isSelected && branchItem.branch === activeBranch

        return (
          <button
            key={branchItem.branch || '__no_branch__'}
            type="button"
            onClick={() => handleBranchClick(isActive ? null : branchItem.branch)}
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
