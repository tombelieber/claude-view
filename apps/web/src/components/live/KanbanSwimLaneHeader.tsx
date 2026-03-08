import { ChevronDown, ChevronRight, FolderOpen, GitBranch } from 'lucide-react'
import { formatCostUsd } from '../../lib/format-utils'
import { cn } from '../../lib/utils'

interface ProjectHeaderProps {
  projectName: string
  totalCostUsd: number
  sessionCount: number
  isCollapsed: boolean
  onToggle: () => void
}

export function ProjectHeader({
  projectName,
  totalCostUsd,
  sessionCount,
  isCollapsed,
  onToggle,
}: ProjectHeaderProps) {
  const Chevron = isCollapsed ? ChevronRight : ChevronDown

  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'w-full flex items-center gap-2 py-2 px-3 cursor-pointer',
        'bg-gray-100/60 dark:bg-gray-800/40',
        'hover:bg-gray-100 dark:hover:bg-gray-800/60',
        'transition-colors',
      )}
    >
      <Chevron className="w-4 h-4 text-gray-400 dark:text-gray-500 shrink-0" />
      <FolderOpen className="w-4 h-4 text-amber-500 dark:text-amber-400 shrink-0" />
      <span className="text-sm font-semibold text-gray-700 dark:text-gray-300 truncate">
        {projectName}
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
        ({sessionCount})
      </span>
      <span className="ml-auto text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums shrink-0">
        {formatCostUsd(totalCostUsd)}
      </span>
    </button>
  )
}

interface BranchHeaderProps {
  branchName: string | null
  sessionCount: number
  isCollapsed: boolean
  onToggle: () => void
}

export function BranchHeader({
  branchName,
  sessionCount,
  isCollapsed,
  onToggle,
}: BranchHeaderProps) {
  const Chevron = isCollapsed ? ChevronRight : ChevronDown

  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'w-full flex items-center gap-2 py-1.5 px-3 pl-6 cursor-pointer',
        'hover:bg-gray-50 dark:hover:bg-gray-800/30',
        'transition-colors',
      )}
    >
      <Chevron className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 shrink-0" />
      <span className="inline-flex items-center gap-1 bg-violet-50 dark:bg-violet-950/50 border border-violet-200 dark:border-violet-800 text-violet-700 dark:text-violet-300 px-1.5 py-0.5 rounded text-xs font-mono">
        <GitBranch className="w-3 h-3 shrink-0" />
        <span className="truncate">{branchName ?? '(no branch)'}</span>
      </span>
      <span className="text-xs text-gray-400 dark:text-gray-500 tabular-nums">
        ({sessionCount})
      </span>
    </button>
  )
}
