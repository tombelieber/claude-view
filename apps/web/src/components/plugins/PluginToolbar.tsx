import { Search } from 'lucide-react'
import { cn } from '../../lib/utils'

interface PluginToolbarProps {
  search: string
  onSearchChange: (value: string) => void
  scope: string | undefined
  onScopeChange: (scope: string | undefined) => void
  source: string | undefined
  onSourceChange: (source: string | undefined) => void
  kind: string | undefined
  onKindChange: (kind: string | undefined) => void
  marketplaces: string[]
  totalCount: number
}

const KIND_TABS = [
  { value: undefined, label: 'All' },
  { value: 'skill', label: 'Skills' },
  { value: 'mcp_tool', label: 'MCP' },
  { value: 'command', label: 'Commands' },
  { value: 'agent', label: 'Agents' },
] as const

const SCOPE_OPTIONS = [
  { value: undefined, label: 'All Scopes' },
  { value: 'user', label: 'User' },
  { value: 'project', label: 'Project' },
  { value: 'available', label: 'Available' },
] as const

export function PluginToolbar({
  search,
  onSearchChange,
  scope,
  onScopeChange,
  source,
  onSourceChange,
  kind,
  onKindChange,
  marketplaces,
  totalCount,
}: PluginToolbarProps) {
  return (
    <div className="px-6 pb-3 flex flex-col gap-2">
      {/* Row 1: Search + Scope + Source */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="relative flex-1 min-w-[180px]">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
          <input
            type="text"
            value={search}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Search plugins..."
            className={cn(
              'w-full pl-8 pr-3 py-1.5 text-sm rounded-md border transition-colors',
              'border-gray-200 dark:border-gray-700',
              'bg-white dark:bg-gray-800/50',
              'text-gray-900 dark:text-gray-100',
              'placeholder:text-gray-400 dark:placeholder:text-gray-500',
              'focus:border-blue-300 dark:focus:border-blue-600 focus:outline-none focus:ring-1 focus:ring-blue-300',
            )}
          />
        </div>

        <select
          value={scope ?? ''}
          onChange={(e) => onScopeChange(e.target.value || undefined)}
          className={cn(
            'text-xs px-2 py-1.5 rounded-md border transition-colors cursor-pointer',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800/50',
            'text-gray-700 dark:text-gray-300',
          )}
        >
          {SCOPE_OPTIONS.map((opt) => (
            <option key={opt.label} value={opt.value ?? ''}>
              {opt.label}
            </option>
          ))}
        </select>

        <select
          value={source ?? ''}
          onChange={(e) => onSourceChange(e.target.value || undefined)}
          className={cn(
            'text-xs px-2 py-1.5 rounded-md border transition-colors cursor-pointer',
            'border-gray-200 dark:border-gray-700',
            'bg-white dark:bg-gray-800/50',
            'text-gray-700 dark:text-gray-300',
          )}
        >
          <option value="">All Sources</option>
          {marketplaces.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>
      </div>

      {/* Row 2: Kind tabs */}
      <div className="flex items-center gap-1">
        {KIND_TABS.map((tab) => (
          <button
            key={tab.label}
            type="button"
            onClick={() => onKindChange(tab.value)}
            className={cn(
              'text-xs px-2.5 py-1 rounded-md transition-colors',
              kind === tab.value
                ? 'bg-blue-500 text-white'
                : 'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
          >
            {tab.label}
            {tab.value === undefined && ` ${totalCount}`}
          </button>
        ))}
      </div>
    </div>
  )
}
