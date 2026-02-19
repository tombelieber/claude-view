import { cn } from '../../../lib/utils'
import type { ActionCategory } from './types'

const CATEGORIES: { id: ActionCategory | 'all'; label: string; color: string }[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'skill', label: 'Skill', color: 'bg-purple-500/10 text-purple-400 border-purple-500/30' },
  { id: 'mcp', label: 'MCP', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'builtin', label: 'Builtin', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'agent', label: 'Agent', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30' },
  { id: 'error', label: 'Error', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
]

interface ActionFilterChipsProps {
  counts: Record<ActionCategory, number>
  activeFilter: ActionCategory | 'all'
  onFilterChange: (filter: ActionCategory | 'all') => void
}

export function ActionFilterChips({ counts, activeFilter, onFilterChange }: ActionFilterChipsProps) {
  const total = Object.values(counts).reduce((a, b) => a + b, 0)

  return (
    <div className="flex items-center gap-1.5 px-3 py-2 overflow-x-auto flex-shrink-0">
      {CATEGORIES.map((cat) => {
        const count = cat.id === 'all' ? total : counts[cat.id] ?? 0
        const isActive = activeFilter === cat.id
        return (
          <button
            key={cat.id}
            onClick={() => onFilterChange(cat.id)}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded-full text-[10px] font-medium border transition-colors cursor-pointer whitespace-nowrap',
              isActive ? cat.color : 'bg-transparent text-gray-500 border-gray-700 hover:border-gray-600',
            )}
          >
            {cat.label}
            {count > 0 && (
              <span className="font-mono tabular-nums">{count}</span>
            )}
          </button>
        )
      })}
    </div>
  )
}
