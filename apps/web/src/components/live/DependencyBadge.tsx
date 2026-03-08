import type { TaskItem } from '../../types/generated/TaskItem'

interface DependencyBadgeProps {
  task: TaskItem
  onScrollTo?: (taskId: string) => void
}

export function DependencyBadge({ task, onScrollTo }: DependencyBadgeProps) {
  const hasBlocks = task.blocks.length > 0
  const hasBlockedBy = task.blockedBy.length > 0

  if (!hasBlocks && !hasBlockedBy) return null

  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 mt-1.5">
      {hasBlocks && (
        <span className="inline-flex items-center gap-1 text-[10px] font-mono text-gray-400 dark:text-gray-500">
          <span aria-hidden>→</span>
          <span>blocks</span>
          {task.blocks.map((id) => (
            <button
              type="button"
              key={id}
              onClick={() => onScrollTo?.(id)}
              className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer"
            >
              #{id}
            </button>
          ))}
        </span>
      )}
      {hasBlockedBy && (
        <span className="inline-flex items-center gap-1 text-[10px] font-mono text-gray-400 dark:text-gray-500">
          <span aria-hidden>←</span>
          <span>blocked by</span>
          {task.blockedBy.map((id) => (
            <button
              type="button"
              key={id}
              onClick={() => onScrollTo?.(id)}
              className="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors cursor-pointer"
            >
              #{id}
            </button>
          ))}
        </span>
      )}
    </div>
  )
}
