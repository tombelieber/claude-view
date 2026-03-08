import { useState } from 'react'
import type { TaskItem } from '../../types/generated/TaskItem'
import { DependencyBadge } from './DependencyBadge'
import { STATUS_CLASS, STATUS_ICON } from './TaskProgressList'

interface TaskDetailCardProps {
  task: TaskItem
  onScrollTo?: (taskId: string) => void
}

export function TaskDetailCard({ task, onScrollTo }: TaskDetailCardProps) {
  const [expanded, setExpanded] = useState(false)
  const icon = STATUS_ICON[task.status] ?? '◻'
  const colorClass = STATUS_CLASS[task.status] ?? STATUS_CLASS.pending

  const isLong = task.description.split('\n').length > 3 || task.description.length > 200
  const showFull = expanded || !isLong

  return (
    <div
      id={`task-card-${task.id}`}
      className="rounded-lg border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900/50 p-3 transition-colors duration-200"
    >
      {/* Header */}
      <div className="flex items-start gap-2">
        <span className={`flex-shrink-0 font-mono mt-0.5 ${colorClass}`}>{icon}</span>
        <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500 mt-0.5 flex-shrink-0">
          #{task.id}
        </span>
        <span
          className={`text-xs font-medium flex-1 min-w-0 ${
            task.status === 'completed'
              ? 'text-gray-400 dark:text-gray-500 line-through'
              : 'text-gray-900 dark:text-gray-100'
          }`}
        >
          {task.subject}
        </span>
        {task.status === 'in_progress' && (
          <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse flex-shrink-0 mt-1.5" />
        )}
      </div>

      {/* ActiveForm */}
      {task.status === 'in_progress' && task.activeForm && (
        <div className="ml-[calc(1em+0.5rem+1.5rem+0.5rem)] mt-0.5 text-[11px] text-blue-600 dark:text-blue-400">
          {task.activeForm}
        </div>
      )}

      {/* Description */}
      {task.description && (
        <div className="mt-2 text-xs text-gray-600 dark:text-gray-400 leading-relaxed">
          <p className={showFull ? '' : 'line-clamp-3'}>{task.description}</p>
          {isLong && (
            <button
              type="button"
              onClick={() => setExpanded(!expanded)}
              className="text-[10px] text-indigo-500 dark:text-indigo-400 hover:underline mt-0.5 cursor-pointer"
            >
              {expanded ? 'Show less' : 'Show more'}
            </button>
          )}
        </div>
      )}

      {/* Dependencies */}
      <DependencyBadge task={task} onScrollTo={onScrollTo} />
    </div>
  )
}
