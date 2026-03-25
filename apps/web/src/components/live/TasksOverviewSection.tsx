import { ListChecks } from 'lucide-react'
import type { ProgressItem } from '../../types/generated/ProgressItem'
import { STATUS_CLASS, STATUS_ICON } from './TaskProgressList'

interface TasksOverviewSectionProps {
  items: ProgressItem[]
}

export function TasksOverviewSection({ items }: TasksOverviewSectionProps) {
  const completed = items.filter((i) => i.status === 'completed').length
  const total = items.length

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-800 bg-gray-50 dark:bg-gray-900 p-3">
      {/* Header */}
      <div className="flex items-center gap-1.5 mb-2">
        <ListChecks className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
        <span className="text-xs font-medium text-gray-500 dark:text-gray-500 uppercase tracking-wide">
          Tasks
        </span>
        <span className="text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums ml-auto">
          {completed}/{total}
        </span>
      </div>

      {/* Progress bar */}
      <div className="h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden mb-3">
        <div
          className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
          style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
        />
      </div>

      {/* Task list */}
      <ul className="space-y-1.5 max-h-[400px] overflow-y-auto">
        {items.map((item, idx) => {
          const icon = STATUS_ICON[item.status] ?? '◻'
          const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
          return (
            <li key={item.id ?? idx} className="flex items-start gap-2 text-xs leading-relaxed">
              <span className={`flex-shrink-0 font-mono mt-0.5 ${colorClass}`}>{icon}</span>
              <div className="min-w-0">
                <span
                  className={
                    item.status === 'completed'
                      ? 'text-gray-400 dark:text-gray-500 line-through'
                      : 'text-gray-700 dark:text-gray-300'
                  }
                >
                  {item.title}
                </span>
                {item.status === 'in_progress' && item.activeForm && (
                  <div className="flex items-center gap-1.5 mt-0.5 text-blue-600 dark:text-blue-400 text-xs">
                    <span className="inline-block w-1.5 h-1.5 rounded-full bg-blue-500 dark:bg-blue-400 animate-pulse flex-shrink-0" />
                    {item.activeForm}
                  </div>
                )}
              </div>
            </li>
          )
        })}
      </ul>
    </div>
  )
}
