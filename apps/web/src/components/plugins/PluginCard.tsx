import { AlertTriangle } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'
import type { PluginInfo, PluginItem } from '../../types/generated'
import { PluginActionMenu } from './PluginActionMenu'
import { formatRelativeTime } from './format-helpers'
import { marketplaceDotColor } from './marketplace-colors'

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function ScopeBadge({ scope }: { scope: string }) {
  const isProject = scope.toLowerCase() === 'project'
  return (
    <span
      className={cn(
        'text-[10px] px-1.5 py-0.5 rounded font-medium uppercase',
        isProject
          ? 'bg-green-50 dark:bg-green-900/30 text-green-600 dark:text-green-400'
          : 'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400',
      )}
    >
      {scope}
    </span>
  )
}

function ContentsLine({ plugin }: { plugin: PluginInfo }) {
  const parts: string[] = []
  if (plugin.skillCount > 0)
    parts.push(`${plugin.skillCount} skill${plugin.skillCount > 1 ? 's' : ''}`)
  if (plugin.agentCount > 0)
    parts.push(`${plugin.agentCount} agent${plugin.agentCount > 1 ? 's' : ''}`)
  if (plugin.commandCount > 0)
    parts.push(`${plugin.commandCount} cmd${plugin.commandCount > 1 ? 's' : ''}`)
  if (plugin.mcpCount > 0) parts.push(`${plugin.mcpCount} MCP`)
  if (parts.length === 0) return null
  return <span className="text-xs text-gray-500 dark:text-gray-500">{parts.join(' \u00b7 ')}</span>
}

function UsageLine({ plugin }: { plugin: PluginInfo }) {
  const invocations = Number(plugin.totalInvocations)
  const sessions = Number(plugin.sessionCount)
  const lastUsed = plugin.lastUsedAt ? formatRelativeTime(Number(plugin.lastUsedAt)) : null
  if (invocations === 0) {
    return <span className="text-xs text-gray-400 dark:text-gray-600">No usage recorded</span>
  }
  return (
    <span className="text-xs text-gray-400 dark:text-gray-600">
      {invocations.toLocaleString()}&times; across {sessions} session{sessions !== 1 ? 's' : ''}
      {lastUsed && ` · ${lastUsed}`}
    </span>
  )
}

function DuplicateWarning({ marketplaces }: { marketplaces: string[] }) {
  if (marketplaces.length === 0) return null
  return (
    <div className="flex items-center gap-1 text-[10px] text-amber-600 dark:text-amber-400">
      <AlertTriangle className="w-3 h-3 flex-shrink-0" />
      Also from {marketplaces.join(', ')}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Expanded items listing
// ---------------------------------------------------------------------------

function ItemRow({ item }: { item: PluginItem }) {
  const count = Number(item.invocationCount)
  const lastUsed = item.lastUsedAt ? formatRelativeTime(Number(item.lastUsedAt)) : null
  return (
    <div className="py-0.5">
      <div className="flex items-center justify-between text-xs">
        <span className="text-gray-700 dark:text-gray-300 truncate">{item.name}</span>
        <span className="text-gray-400 dark:text-gray-500 whitespace-nowrap ml-2">
          {count > 0 ? `${count}\u00d7` : '\u2014'}
          {lastUsed && <span className="ml-2">{lastUsed}</span>}
        </span>
      </div>
      {item.description && (
        <div className="text-[10px] text-gray-400 dark:text-gray-500 truncate mt-0.5">
          {item.description}
        </div>
      )}
    </div>
  )
}

function ItemsSection({ kind, items }: { kind: string; items: PluginItem[] }) {
  const [expanded, setExpanded] = useState(false)
  if (items.length === 0) return null
  const visible = expanded ? items : items.slice(0, 5)
  const remaining = items.length - 5
  return (
    <div className="mt-2">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-gray-400 dark:text-gray-500 mb-0.5">
        {kind} ({items.length})
      </div>
      {visible.map((item) => (
        <ItemRow key={item.id} item={item} />
      ))}
      {remaining > 0 && !expanded && (
        <button
          type="button"
          onClick={(e) => {
            e.stopPropagation()
            setExpanded(true)
          }}
          className="text-[10px] text-blue-500 hover:text-blue-600 mt-0.5"
        >
          +{remaining} more
        </button>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main card
// ---------------------------------------------------------------------------

interface PluginCardProps {
  plugin: PluginInfo
  onAction: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
}

export function PluginCard({ plugin, onAction, isPending }: PluginCardProps) {
  const [expanded, setExpanded] = useState(false)

  const skills = plugin.items.filter((i) => i.kind === 'skill')
  const agents = plugin.items.filter((i) => i.kind === 'agent')
  const commands = plugin.items.filter((i) => i.kind === 'command')
  const mcpTools = plugin.items.filter((i) => i.kind === 'mcp_tool')

  const version = plugin.gitSha ? plugin.gitSha.slice(0, 6) : plugin.version

  return (
    <button
      type="button"
      className={cn(
        'group w-full text-left rounded-lg border p-3 transition-colors duration-200 cursor-pointer',
        'border-gray-200 dark:border-gray-800',
        'bg-white dark:bg-gray-900/50',
        'hover:border-gray-300 dark:hover:border-gray-700',
        !plugin.enabled && 'opacity-50',
      )}
      onClick={() => setExpanded(!expanded)}
    >
      {/* Row 1: name + scope + menu */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 truncate">
            {plugin.name}
          </h3>
          {!plugin.enabled && (
            <span className="text-[10px] px-1.5 py-0.5 rounded font-medium uppercase bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 flex-shrink-0">
              Disabled
            </span>
          )}
        </div>
        <div className="flex items-center gap-1.5 flex-shrink-0">
          <ScopeBadge scope={plugin.scope} />
          <PluginActionMenu plugin={plugin} onAction={onAction} isPending={isPending} />
        </div>
      </div>

      {/* Row 2: marketplace + version + install count */}
      <div className="flex items-center gap-2 mt-1 text-[10px] text-gray-500 dark:text-gray-500">
        <span className="flex items-center gap-1">
          <span
            className={cn(
              'w-2 h-2 rounded-full inline-block',
              marketplaceDotColor(plugin.marketplace),
            )}
          />
          {plugin.marketplace}
        </span>
        {version && (
          <>
            <span className="text-gray-300 dark:text-gray-700">&middot;</span>
            <span className="font-mono">{version}</span>
          </>
        )}
        {plugin.installCount != null && (
          <>
            <span className="text-gray-300 dark:text-gray-700">&middot;</span>
            <span>{formatInstallCount(plugin.installCount)} installs</span>
          </>
        )}
      </div>

      {/* Row 3: description (if available) */}
      {plugin.description && (
        <p className="text-xs text-gray-500 dark:text-gray-400 mt-1.5 line-clamp-2">
          {plugin.description}
        </p>
      )}

      {/* Row 4: contents + usage */}
      <div className="mt-2 flex flex-col gap-0.5">
        <ContentsLine plugin={plugin} />
        <UsageLine plugin={plugin} />
      </div>

      {/* Duplicate warning */}
      {plugin.duplicateMarketplaces.length > 0 && (
        <div className="mt-2">
          <DuplicateWarning marketplaces={plugin.duplicateMarketplaces} />
        </div>
      )}

      {/* Error display */}
      {plugin.errors.length > 0 && (
        <div className="mt-2 text-[10px] text-red-500 dark:text-red-400">
          {plugin.errors.map((err) => (
            <div key={err}>{err}</div>
          ))}
        </div>
      )}

      {/* Expanded: detailed items listing */}
      {expanded && (
        <div className="mt-3 pt-3 border-t border-gray-100 dark:border-gray-800">
          <div className="text-[10px] text-gray-400 dark:text-gray-500 mb-1">
            Installed {plugin.installedAt.split('T')[0]}
            {plugin.lastUpdated && ` \u00b7 Updated ${plugin.lastUpdated.split('T')[0]}`}
            {plugin.gitSha && ` \u00b7 SHA: ${plugin.gitSha.slice(0, 12)}`}
          </div>
          <ItemsSection kind="Skills" items={skills} />
          <ItemsSection kind="Agents" items={agents} />
          <ItemsSection kind="Commands" items={commands} />
          <ItemsSection kind="MCP Tools" items={mcpTools} />
        </div>
      )}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatInstallCount(count: bigint): string {
  const n = Number(count)
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toString()
}
