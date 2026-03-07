import { Layers } from 'lucide-react'
import type { KanbanGroupBy } from './types'

interface KanbanGroupByControlProps {
  value: KanbanGroupBy
  onChange: (value: KanbanGroupBy) => void
}

const OPTIONS: { value: KanbanGroupBy; label: string }[] = [
  { value: 'none', label: 'None' },
  { value: 'project-branch', label: 'Project + Branch' },
]

export function KanbanGroupByControl({ value, onChange }: KanbanGroupByControlProps) {
  return (
    <div className="flex items-center gap-2 text-xs text-gray-500 dark:text-gray-400">
      <Layers className="w-3.5 h-3.5" />
      <span>Group by:</span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as KanbanGroupBy)}
        className="bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-md px-2 py-1 text-xs text-gray-700 dark:text-gray-300 cursor-pointer focus:outline-none focus:border-indigo-500 focus:ring-1 focus:ring-indigo-500/30"
      >
        {OPTIONS.map((opt) => (
          <option key={opt.value} value={opt.value}>
            {opt.label}
          </option>
        ))}
      </select>
    </div>
  )
}
