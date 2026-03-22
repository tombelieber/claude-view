import { cn } from '../../../lib/utils'

/**
 * Generic filter chips — same visual language, any category type.
 * Used by both ActionLog (ActionCategory) and ConversationThread (block types).
 */

export interface ChipDefinition<T extends string> {
  id: T | 'all'
  label: string
  color: string
}

interface FilterChipsProps<T extends string> {
  categories: ChipDefinition<T>[]
  counts: Record<T, number>
  activeFilter: T[] | 'all'
  onFilterChange: (category: T | 'all') => void
}

export function FilterChips<T extends string>({
  categories,
  counts,
  activeFilter,
  onFilterChange,
}: FilterChipsProps<T>) {
  const total = Object.values(counts).reduce((a, b) => (a as number) + (b as number), 0) as number

  return (
    <div className="flex items-center gap-1.5 px-3 py-2 overflow-x-auto flex-shrink-0">
      {categories.map((cat) => {
        const count = cat.id === 'all' ? total : (counts[cat.id as T] ?? 0)
        const isActive =
          activeFilter === 'all'
            ? cat.id === 'all'
            : cat.id !== 'all' && activeFilter.includes(cat.id as T)
        return (
          <button
            key={cat.id}
            onClick={() => onFilterChange(cat.id as T | 'all')}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium border transition-colors cursor-pointer whitespace-nowrap',
              isActive
                ? cat.color
                : 'bg-transparent text-gray-500 border-gray-700 hover:border-gray-600',
            )}
          >
            {cat.label}
            {count > 0 && <span className="font-mono tabular-nums">{count}</span>}
          </button>
        )
      })}
    </div>
  )
}
