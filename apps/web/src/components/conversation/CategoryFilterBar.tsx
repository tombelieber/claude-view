import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useMemo } from 'react'

type CategoryKey = ConversationBlock['type']

const LABEL_MAP: Record<CategoryKey, string> = {
  user: 'User',
  assistant: 'Assistant',
  interaction: 'Prompt',
  turn_boundary: 'Turn',
  notice: 'Notice',
  system: 'System',
  progress: 'Progress',
}

interface CategoryFilterBarProps {
  blocks: ConversationBlock[]
  activeFilter: CategoryKey | null
  onFilterChange: (filter: CategoryKey | null) => void
}

export function CategoryFilterBar({
  blocks,
  activeFilter,
  onFilterChange,
}: CategoryFilterBarProps) {
  const counts = useMemo(() => {
    const map = new Map<CategoryKey, number>()
    for (const block of blocks) {
      map.set(block.type, (map.get(block.type) ?? 0) + 1)
    }
    return map
  }, [blocks])

  const activeClass = 'bg-blue-500 text-white'
  const inactiveClass =
    'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
  const chipBase = 'text-[10px] px-2 py-0.5 rounded-full cursor-pointer'

  return (
    <div className="flex flex-wrap gap-1.5" data-testid="category-filter-bar">
      <button
        type="button"
        className={`${chipBase} ${activeFilter === null ? activeClass : inactiveClass}`}
        onClick={() => onFilterChange(null)}
      >
        All ({blocks.length})
      </button>
      {Array.from(counts.entries()).map(([key, count]) => (
        <button
          key={key}
          type="button"
          className={`${chipBase} ${activeFilter === key ? activeClass : inactiveClass}`}
          onClick={() => onFilterChange(key)}
        >
          {LABEL_MAP[key]} ({count})
        </button>
      ))}
    </div>
  )
}
