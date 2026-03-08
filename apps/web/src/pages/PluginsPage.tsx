import { Blocks } from 'lucide-react'
import { useMemo, useState } from 'react'
import { AvailablePluginCard } from '../components/plugins/AvailablePluginCard'
import { MarketplacesDialog } from '../components/plugins/MarketplacesDialog'
import { PluginCard } from '../components/plugins/PluginCard'
import { PluginHealthBanner } from '../components/plugins/PluginHealthBanner'
import { PluginToolbar } from '../components/plugins/PluginToolbar'
import { usePluginMutations } from '../hooks/use-plugin-mutations'
import { usePlugins } from '../hooks/use-plugins'

export function PluginsPage() {
  const [search, setSearch] = useState('')
  const [scope, setScope] = useState<string | undefined>()
  const [source, setSource] = useState<string | undefined>()
  const [kind, setKind] = useState<string | undefined>()

  // Debounce search to avoid thrashing the API
  const [debouncedSearch, setDebouncedSearch] = useState('')
  useMemo(() => {
    const id = setTimeout(() => setDebouncedSearch(search), 300)
    return () => clearTimeout(id)
  }, [search])

  const { data, isLoading, error } = usePlugins({
    search: debouncedSearch || undefined,
    scope,
    source,
    kind,
    sort: 'usage',
  })

  const mutations = usePluginMutations()

  const handleAction = (action: string, name: string, actionScope?: string) => {
    mutations.execute({ action, name, scope: actionScope ?? null })
  }

  const handleUpdateAll = async () => {
    if (!data) return
    const updatable = data.installed.filter((p) => p.updatable)
    for (const plugin of updatable) {
      await mutations.execute({ action: 'update', name: plugin.name, scope: null })
    }
  }

  const totalCount = (data?.totalInstalled ?? 0) + (data?.totalAvailable ?? 0)

  return (
    <div className="h-full flex flex-col overflow-y-auto">
      {/* Header */}
      <div className="px-6 pt-6 pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Blocks className="w-5 h-5 text-blue-500" />
            <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Plugins</h1>
            {data && (
              <span className="text-xs text-gray-400 dark:text-gray-500">{totalCount} total</span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <MarketplacesDialog />
            {data && data.updatableCount > 0 && (
              <button
                type="button"
                onClick={handleUpdateAll}
                disabled={mutations.isPending}
                className="text-xs px-3 py-1.5 rounded-md bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 hover:bg-blue-100 dark:hover:bg-blue-900/50 transition-colors disabled:opacity-50"
              >
                Update All ({data.updatableCount})
              </button>
            )}
          </div>
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

      {/* Health banner */}
      {data && (
        <PluginHealthBanner
          duplicateCount={data.duplicateCount}
          unusedCount={data.unusedCount}
          cliError={data.cliError}
        />
      )}

      {/* Content */}
      <div className="flex-1 px-6 pb-6">
        {isLoading && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {Array.from({ length: 6 }).map((_, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: static skeleton placeholders
              <div key={i} className="h-28 rounded-lg bg-gray-100 dark:bg-gray-800 animate-pulse" />
            ))}
          </div>
        )}

        {error && (
          <div className="text-sm text-red-500">Failed to load plugins: {error.message}</div>
        )}

        {data && totalCount === 0 && !data.cliError && (
          <div className="flex flex-col items-center justify-center py-16 text-gray-400 dark:text-gray-500">
            <Blocks className="w-10 h-10 mb-3 opacity-40" />
            <p className="text-sm font-medium">No plugins found</p>
            <p className="text-xs mt-1">
              Install plugins with <code className="font-mono">claude plugin install</code>
            </p>
          </div>
        )}

        {data && totalCount > 0 && (
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
            {data.installed.map((plugin) => (
              <PluginCard
                key={plugin.id}
                plugin={plugin}
                onAction={handleAction}
                isPending={mutations.isPending}
              />
            ))}
            {data.available.map((plugin) => (
              <AvailablePluginCard
                key={plugin.pluginId}
                plugin={plugin}
                onInstall={(name, installScope) => handleAction('install', name, installScope)}
                isPending={mutations.isPending}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
