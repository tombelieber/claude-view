import { useCallback } from 'react'
import type { TaskItem } from '../../types/generated/TaskItem'
import { TaskDetailCard } from './TaskDetailCard'
import { STATUS_CLASS, STATUS_ICON } from './TaskProgressList'

interface TaskDetailTabProps {
  tasks: TaskItem[]
}

export function TaskDetailTab({ tasks }: TaskDetailTabProps) {
  const completed = tasks.filter((t) => t.status === 'completed').length
  const inProgress = tasks.filter((t) => t.status === 'in_progress').length
  const pending = tasks.filter((t) => t.status === 'pending').length
  const total = tasks.length
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0

  const handleScrollTo = useCallback((taskId: string) => {
    const el = document.getElementById(`task-card-${taskId}`)
    el?.scrollIntoView({ behavior: 'smooth', block: 'center' })
  }, [])

  return (
    <div className="p-4 overflow-y-auto h-full space-y-3">
      {/* Progress bar */}
      <div>
        <div className="flex items-center gap-2 mb-1">
          <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
              style={{ width: `${pct}%` }}
            />
          </div>
          <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 tabular-nums">
            {completed}/{total} completed ({pct}%)
          </span>
        </div>

        {/* Status chips */}
        <div className="flex items-center gap-3 text-xs">
          <span className={`inline-flex items-center gap-1 ${STATUS_CLASS.completed}`}>
            <span className="font-mono">{STATUS_ICON.completed}</span> {completed}
          </span>
          <span className={`inline-flex items-center gap-1 ${STATUS_CLASS.in_progress}`}>
            <span className="font-mono">{STATUS_ICON.in_progress}</span> {inProgress}
          </span>
          <span className={`inline-flex items-center gap-1 ${STATUS_CLASS.pending}`}>
            <span className="font-mono">{STATUS_ICON.pending}</span> {pending}
          </span>
        </div>
      </div>

      {/* Task cards */}
      <div className="space-y-2">
        {tasks.map((task) => (
          <TaskDetailCard key={task.id} task={task} onScrollTo={handleScrollTo} />
        ))}
      </div>
    </div>
  )
}
