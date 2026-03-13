import { Search } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { MarketplaceInfo } from '../../types/generated'

interface KindCounts {
  plugin: number
  skill: number
  command: number
  agent: number
  mcp_tool: number
}

interface PluginToolbarProps {
  search: string
  onSearchChange: (value: string) => void
  scope: string | undefined
  onScopeChange: (scope: string | undefined) => void
  source: string | undefined
  onSourceChange: (source: string | undefined) => void
  kind: string | undefined
  onKindChange: (kind: string | undefined) => void
  marketplaces: MarketplaceInfo[]
  totalCount: number
  kindCounts: KindCounts
}

const KIND_TABS = [
  { value: undefined, label: 'All' },
  { value: 'plugin', label: 'Plugins' },
  { value: 'skill', label: 'Skills' },
  { value: 'command', label: 'Commands' },
  { value: 'agent', label: 'Agents' },
  { value: 'mcp_tool', label: 'MCP' },
] as const

// Tabs whose items are user-owned (displayed in indigo to signal "yours")
const LOCAL_KIND_VALUES = new Set(['skill', 'command', 'agent'])

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
  kindCounts,
}: PluginToolbarProps) {
  return (
    <div className="px-6 pb-3 flex flex-col gap-2">
      {/* Row 1: Search + Scope + Source */}
      <div className="flex items-center gap-2 flex-wrap">
        <div className="relative flex-1 min-w-[180px]">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-4 h-4 text-apple-text3" />
          <input
            type="text"
            value={search}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Search plugins..."
            className="w-full pl-8 pr-3 py-2 text-[13px] rounded-[10px] border border-apple-sep bg-white text-apple-text1 placeholder:text-apple-text3 focus:border-apple-blue focus:outline-none focus:ring-[3px] focus:ring-[rgba(0,122,255,0.12)]"
          />
        </div>

        <select
          value={scope ?? ''}
          onChange={(e) => onScopeChange(e.target.value || undefined)}
          className="text-[13px] px-2.5 py-1.5 rounded-lg border border-apple-sep bg-white text-apple-text2 cursor-pointer outline-none"
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
          className="text-[13px] px-2.5 py-1.5 rounded-lg border border-apple-sep bg-white text-apple-text2 cursor-pointer outline-none"
        >
          <option value="">All Sources</option>
          {marketplaces.map((m) => (
            <option key={m.name} value={m.name}>
              {m.name}
            </option>
          ))}
        </select>
      </div>

      {/* Row 2: Kind tabs */}
      <div className="flex items-center gap-0.5 bg-apple-sep2 rounded-[9px] p-0.5 w-fit">
        {KIND_TABS.map((tab) => {
          const isLocal = LOCAL_KIND_VALUES.has(tab.value ?? '')
          const isActive = kind === tab.value
          return (
            <button
              key={tab.label}
              type="button"
              onClick={() => onKindChange(tab.value)}
              className={cn(
                'text-[12px] font-medium px-3 py-1.5 rounded-[7px] border-none cursor-pointer',
                'bg-transparent transition-all duration-150',
                isActive
                  ? cn(
                      'bg-white shadow-[0_1px_3px_rgba(0,0,0,0.1)]',
                      isLocal ? 'text-apple-indigo' : 'text-apple-text1',
                    )
                  : cn(
                      'hover:text-apple-text1',
                      isLocal ? 'text-apple-indigo' : 'text-apple-text2',
                    ),
              )}
            >
              {tab.value === undefined
                ? `${tab.label} (${totalCount})`
                : `${tab.label} (${kindCounts[tab.value]})`}
            </button>
          )
        })}
      </div>
    </div>
  )
}
