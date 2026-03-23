import { type ChipDefinition, FilterChips } from '../../ui/FilterChips'
import type { ActionCategory } from './types'

const ACTION_CATEGORIES: ChipDefinition<ActionCategory>[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'builtin', label: 'Builtin', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'mcp', label: 'MCP', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'agent', label: 'Agent', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30' },
  { id: 'skill', label: 'Skill', color: 'bg-purple-500/10 text-purple-400 border-purple-500/30' },
  { id: 'hook', label: 'Hook', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' },
  {
    id: 'hook_progress',
    label: 'Hook Progress',
    color: 'bg-yellow-500/10 text-yellow-400 border-yellow-500/30',
  },
  { id: 'system', label: 'System', color: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30' },
  { id: 'snapshot', label: 'Snapshot', color: 'bg-teal-500/10 text-teal-400 border-teal-500/30' },
  { id: 'queue', label: 'Queue', color: 'bg-orange-500/10 text-orange-400 border-orange-500/30' },
  { id: 'error', label: 'Error', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
]

interface ActionFilterChipsProps {
  counts: Record<ActionCategory, number>
  activeFilter: ActionCategory[] | 'all'
  onFilterChange: (category: ActionCategory | 'all') => void
}

export function ActionFilterChips(props: ActionFilterChipsProps) {
  return <FilterChips categories={ACTION_CATEGORIES} {...props} />
}
