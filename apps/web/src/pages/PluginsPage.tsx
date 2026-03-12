import { Blocks, TrendingUp } from 'lucide-react'
import { useEffect, useState } from 'react'
import { AvailableSection } from '../components/plugins/AvailableSection'
import { InstalledPluginsSection } from '../components/plugins/InstalledPluginsSection'
import { MarketplacesDialog } from '../components/plugins/MarketplacesDialog'
import { PluginHealthPanel } from '../components/plugins/PluginHealthPanel'
import { PluginToolbar } from '../components/plugins/PluginToolbar'
import { UserItemSection } from '../components/plugins/UserItemSection'
import { usePluginMutations } from '../hooks/use-plugin-mutations'
import { usePlugins } from '../hooks/use-plugins'

export function PluginsPage() {
  const [search, setSearch] = useState('')
  const [scope, setScope] = useState<string | undefined>()
  const [source, setSource] = useState<string | undefined>()
  const [kind, setKind] = useState<string | undefined>()

  // Debounce search to avoid thrashing the API
  const [debouncedSearch, setDebouncedSearch] = useState('')
  useEffect(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 300)
    return () => clearTimeout(id)
  }, [search])

  const { data } = usePlugins({
    search: debouncedSearch || undefined,
    scope,
    source,
    kind,
    sort: 'usage',
  })

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

  const handleUpdateAll = async () => {
    if (!data) return
    const updatable = data.installed.filter((p) => p.updatable)
    for (const plugin of updatable) {
      await mutations.execute({
        action: 'update',
        name: plugin.name,
        scope: plugin.scope,
        projectPath: plugin.projectPath ?? null,
      })
    }
  }

  const totalCount =
    (data?.totalInstalled ?? 0) +
    (data?.totalAvailable ?? 0) +
    (data?.userSkills?.length ?? 0) +
    (data?.userCommands?.length ?? 0) +
    (data?.userAgents?.length ?? 0)

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
          {data && data.updatableCount > 0 && (
            <button
              type="button"
              onClick={handleUpdateAll}
              disabled={mutations.isPending}
              className="inline-flex items-center gap-1.5 text-[13px] font-medium px-3.5 py-1.5 rounded-lg bg-apple-blue text-white hover:opacity-85 transition-opacity disabled:opacity-50"
            >
              <TrendingUp className="w-3 h-3" />
              Update All ({data.updatableCount})
            </button>
          )}
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
        marketplaces={data?.marketplaces ?? []}
        totalCount={totalCount}
      />

      {/* Health panel */}
      {data && (
        <PluginHealthPanel
          orphanCount={data.orphanCount}
          conflictCount={data.duplicateCount}
          unusedCount={data.unusedCount}
          cliError={data.cliError}
        />
      )}

      {/* Content sections */}
      <div className="px-7 py-5 flex flex-col gap-7">
        {(data?.userSkills?.length ?? 0) > 0 && (
          <UserItemSection title="Skills" items={data!.userSkills} pathPrefix="~/.claude/skills/" />
        )}
        {(data?.userCommands?.length ?? 0) > 0 && (
          <UserItemSection
            title="Commands"
            items={data!.userCommands}
            pathPrefix="~/.claude/commands/"
          />
        )}
        {(data?.userAgents?.length ?? 0) > 0 && (
          <UserItemSection title="Agents" items={data!.userAgents} pathPrefix="~/.claude/agents/" />
        )}
        {(data?.installed?.length ?? 0) > 0 && (
          <InstalledPluginsSection
            plugins={data!.installed}
            onAction={handleAction}
            isPending={mutations.isPending}
          />
        )}
        {(data?.available?.length ?? 0) > 0 && (
          <AvailableSection
            plugins={data!.available}
            onInstall={(name, scope) => handleAction('install', name, scope)}
            isPending={mutations.isPending}
          />
        )}

        {/* Empty state */}
        {data && totalCount === 0 && !data.cliError && (
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
