import type { ProgressItem } from '../../types/generated/ProgressItem'

const STATUS_ICON: Record<string, string> = {
  pending: '◻',
  in_progress: '◼',
  completed: '✓',
}

const STATUS_CLASS: Record<string, string> = {
  pending: 'text-gray-400 dark:text-gray-500',
  in_progress: 'text-gray-600 dark:text-gray-300',
  completed: 'text-green-500 dark:text-green-400',
}

interface TaskProgressListProps {
  items: ProgressItem[]
}

export function TaskProgressList({ items }: TaskProgressListProps) {
  if (items.length === 0) return null

  const completed = items.filter(i => i.status === 'completed').length
  const total = items.length

  return (
    <div className="mb-2">
      <div className="flex items-center gap-1.5 mb-1">
        <span className="text-[10px] font-medium text-gray-500 dark:text-gray-400">
          Tasks {completed}/{total}
        </span>
        {/* Mini progress bar */}
        <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
          <div
            className="h-full bg-green-500 dark:bg-green-400 rounded-full transition-all"
            style={{ width: `${total > 0 ? (completed / total) * 100 : 0}%` }}
          />
        </div>
      </div>
      <ul className="space-y-0.5">
        {items.slice(0, 5).map((item, idx) => {
          const icon = STATUS_ICON[item.status] ?? '◻'
          const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
          const label = item.status === 'in_progress' && item.activeForm
            ? item.activeForm
            : item.title
          return (
            <li key={item.id ?? idx} className="flex items-start gap-1.5 text-xs leading-tight">
              <span className={`flex-shrink-0 font-mono ${colorClass}`}>{icon}</span>
              <span className={`truncate ${item.status === 'completed' ? 'text-gray-400 dark:text-gray-500 line-through' : 'text-gray-600 dark:text-gray-300'}`}>
                {label}
              </span>
            </li>
          )
        })}
        {items.length > 5 && (
          <li className="text-[10px] text-gray-400 dark:text-gray-500 pl-4">
            +{items.length - 5} more
          </li>
        )}
      </ul>
    </div>
  )
}
