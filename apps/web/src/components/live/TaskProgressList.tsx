import * as Tooltip from '@radix-ui/react-tooltip'
import type { ProgressItem } from '../../types/generated/ProgressItem'

export const STATUS_ICON: Record<string, string> = {
  pending: '◻',
  in_progress: '◼',
  completed: '✓',
}

export const STATUS_CLASS: Record<string, string> = {
  pending: 'text-gray-400 dark:text-gray-500',
  in_progress: 'text-gray-600 dark:text-gray-300',
  completed: 'text-green-500 dark:text-green-400',
}

const TOOLTIP_CONTENT_CLASS =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-sm text-xs'
const TOOLTIP_ARROW_CLASS = 'fill-gray-200 dark:fill-gray-700'

interface TaskProgressListProps {
  items: ProgressItem[]
}

export function TaskProgressList({ items }: TaskProgressListProps) {
  if (items.length === 0) return null

  const completed = items.filter((i) => i.status === 'completed').length
  const total = items.length

  return (
    <Tooltip.Provider delayDuration={200}>
      <Tooltip.Root>
        <Tooltip.Trigger asChild>
          <div className="mb-2 cursor-default">
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
                const label =
                  item.status === 'in_progress' && item.activeForm ? item.activeForm : item.title
                return (
                  <li
                    key={item.id ?? idx}
                    className="flex items-start gap-1.5 text-xs leading-tight"
                  >
                    <span className={`flex-shrink-0 font-mono ${colorClass}`}>{icon}</span>
                    <span
                      className={`truncate ${item.status === 'completed' ? 'text-gray-400 dark:text-gray-500 line-through' : 'text-gray-600 dark:text-gray-300'}`}
                    >
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
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
            <div className="font-medium text-gray-900 dark:text-gray-100 mb-2">
              Tasks {completed}/{total}
            </div>
            <ul className="space-y-1 max-h-64 overflow-y-auto">
              {items.map((item, idx) => {
                const icon = STATUS_ICON[item.status] ?? '◻'
                const colorClass = STATUS_CLASS[item.status] ?? STATUS_CLASS.pending
                const label =
                  item.status === 'in_progress' && item.activeForm ? item.activeForm : item.title
                return (
                  <li key={item.id ?? idx} className="flex items-start gap-1.5 leading-tight">
                    <span className={`flex-shrink-0 font-mono ${colorClass}`}>{icon}</span>
                    <span
                      className={`${item.status === 'completed' ? 'text-gray-400 dark:text-gray-500 line-through' : 'text-gray-700 dark:text-gray-300'}`}
                    >
                      {label}
                    </span>
                  </li>
                )
              })}
            </ul>
            <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  )
}
