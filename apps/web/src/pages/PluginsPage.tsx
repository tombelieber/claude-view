import { Blocks, TrendingUp } from 'lucide-react'
import { useMemo, useState } from 'react'
import { AvailableSection } from '../components/plugins/AvailableSection'
import { InstalledPluginsSection } from '../components/plugins/InstalledPluginsSection'
import { MarketplacesDialog } from '../components/plugins/MarketplacesDialog'
import { PluginHealthPanel } from '../components/plugins/PluginHealthPanel'
import { PluginToolbar } from '../components/plugins/PluginToolbar'
import { PluginsPageSkeleton } from '../components/plugins/PluginsPageSkeleton'
import { UserItemSection } from '../components/plugins/UserItemSection'
import { usePluginMutations } from '../hooks/use-plugin-mutations'
import { usePlugins } from '../hooks/use-plugins'

export function PluginsPage() {
  const [search, setSearch] = useState('')
  const [scope, setScope] = useState<string | undefined>()
  const [source, setSource] = useState<string | undefined>()
  const [kind, setKind] = useState<string | undefined>('plugin')

  const { data } = usePlugins({
    scope,
    source,
    kind,
    sort: 'installs',
  })

  // Frontend-only name filter — instant, no API round-trip
  const needle = search.toLowerCase()
  const filteredInstalled = useMemo(
    () =>
      needle
        ? (data?.installed ?? []).filter((p) => p.name.toLowerCase().includes(needle))
        : (data?.installed ?? []),
    [data?.installed, needle],
  )
  const filteredAvailable = useMemo(
    () =>
      needle
        ? (data?.available ?? []).filter((p) => p.name.toLowerCase().includes(needle))
        : (data?.available ?? []),
    [data?.available, needle],
  )
  const filteredSkills = useMemo(
    () =>
      needle
        ? (data?.userSkills ?? []).filter((i) => i.name.toLowerCase().includes(needle))
        : (data?.userSkills ?? []),
    [data?.userSkills, needle],
  )
  const filteredCommands = useMemo(
    () =>
      needle
        ? (data?.userCommands ?? []).filter((i) => i.name.toLowerCase().includes(needle))
        : (data?.userCommands ?? []),
    [data?.userCommands, needle],
  )
  const filteredAgents = useMemo(
    () =>
      needle
        ? (data?.userAgents ?? []).filter((i) => i.name.toLowerCase().includes(needle))
        : (data?.userAgents ?? []),
    [data?.userAgents, needle],
  )

  const mutations = usePluginMutations()

  const handleAction = (
    action: string,
    name: string,
    actionScope?: string,
    projectPath?: string | null,
  ) => {
    mutations.execute({
      action,
      name,
      scope: actionScope ?? null,
      projectPath: projectPath ?? null,
    })
  }

  const handleUpdateAll = () => {
    if (!data) return
    const updatable = data.installed.filter((p) => p.updatable)
    for (const plugin of updatable) {
      mutations.execute({
        action: 'update',
        name: plugin.name,
        scope: plugin.scope,
        projectPath: plugin.projectPath ?? null,
      })
    }
  }

  if (!data) return <PluginsPageSkeleton />

  const totalCount =
    (data.totalInstalled ?? 0) +
    (data.totalAvailable ?? 0) +
    (data.userSkills?.length ?? 0) +
    (data.userCommands?.length ?? 0) +
    (data.userAgents?.length ?? 0)

  const kindCounts = {
    plugin: (data.totalInstalled ?? 0) + (data.totalAvailable ?? 0),
    skill: data.userSkills?.length ?? 0,
    command: data.userCommands?.length ?? 0,
    agent: data.userAgents?.length ?? 0,
    mcp_tool: data.installed?.reduce((s, p) => s + p.mcpCount, 0) ?? 0,
  }

  return (
    <div className="min-h-full bg-apple-bg">
      {/* Header */}
      <div className="px-7 pt-6 pb-0 flex items-start justify-between gap-4">
        <div>
          <h1 className="text-[28px] font-bold tracking-[-0.4px] text-apple-text1">Plugins</h1>
          <p className="text-[13px] text-apple-text3 mt-0.5">
            {totalCount} items — skills, commands, agents &amp; installed plugins
          </p>
        </div>
        <div className="flex gap-2 items-center pt-1.5">
          <MarketplacesDialog />
          <button
            type="button"
            onClick={handleUpdateAll}
            disabled={data.updatableCount === 0 || mutations.isPending}
            className="inline-flex items-center gap-1.5 text-[13px] font-medium px-3.5 py-1.5 rounded-lg bg-apple-blue text-white hover:opacity-85 transition-opacity disabled:opacity-40"
          >
            <TrendingUp className="w-3 h-3" />
            {data.updatableCount > 0 ? `Update All (${data.updatableCount})` : 'Update All'}
          </button>
        </div>
      </div>

      {/* Toolbar */}
      <PluginToolbar
        search={search}
        onSearchChange={setSearch}
        scope={scope}
        onScopeChange={setScope}
        source={source}
        onSourceChange={setSource}
        kind={kind}
        onKindChange={setKind}
        marketplaces={data.marketplaces ?? []}
        totalCount={totalCount}
        kindCounts={kindCounts}
      />

      {/* Health panel */}
      <PluginHealthPanel
        orphanCount={data.orphanCount}
        conflictCount={data.duplicateCount}
        unusedCount={data.unusedCount}
        cliError={data.cliError}
      />

      {/* Content sections */}
      <div className="px-7 py-5 flex flex-col gap-7">
        {/* Skills section — only when kind is undefined or 'skill' */}
        {(!kind || kind === 'skill') && filteredSkills.length > 0 && (
          <UserItemSection title="Skills" items={filteredSkills} pathPrefix="~/.claude/skills/" />
        )}
        {/* Commands section */}
        {(!kind || kind === 'command') && filteredCommands.length > 0 && (
          <UserItemSection
            title="Commands"
            items={filteredCommands}
            pathPrefix="~/.claude/commands/"
          />
        )}
        {/* Agents section */}
        {(!kind || kind === 'agent') && filteredAgents.length > 0 && (
          <UserItemSection title="Agents" items={filteredAgents} pathPrefix="~/.claude/agents/" />
        )}
        {/* Installed plugins — shown for 'plugin', 'mcp_tool', or all */}
        {(!kind || kind === 'plugin' || kind === 'mcp_tool') && filteredInstalled.length > 0 && (
          <InstalledPluginsSection
            plugins={filteredInstalled}
            onAction={handleAction}
            isPluginPending={mutations.isPluginPending}
            marketplaces={data.marketplaces}
          />
        )}
        {/* Available plugins — only for 'plugin' or all */}
        {(!kind || kind === 'plugin') && filteredAvailable.length > 0 && (
          <AvailableSection
            plugins={filteredAvailable}
            onInstall={(name, scope) => handleAction('install', name, scope)}
            isPluginPending={mutations.isPluginPending}
            marketplaces={data.marketplaces}
          />
        )}

        {/* Empty state */}
        {totalCount === 0 && !data.cliError && (
          <div className="flex flex-col items-center justify-center py-16 text-apple-text3">
            <Blocks className="w-10 h-10 mb-3 opacity-40" />
            <p className="text-sm font-medium">No plugins found</p>
            <p className="text-xs mt-1">
              Install plugins with <code className="font-mono">claude plugin install</code>
            </p>
          </div>
        )}
      </div>
    </div>
  )
}
