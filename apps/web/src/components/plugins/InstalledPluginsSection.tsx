import type { MarketplaceInfo, PluginInfo } from '../../types/generated'
import { PluginCard } from './PluginCard'
import { SectionHeader } from './SectionHeader'

interface InstalledPluginsSectionProps {
  plugins: PluginInfo[]
  onAction: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPluginPending: (name: string) => boolean
  marketplaces?: MarketplaceInfo[]
}

export function InstalledPluginsSection({
  plugins,
  onAction,
  isPluginPending,
  marketplaces = [],
}: InstalledPluginsSectionProps) {
  return (
    <div>
      <SectionHeader title="Installed Plugins" count={plugins.length} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {plugins.map((plugin) => {
          const mp = marketplaces.find((m) => m.name === plugin.marketplace)
          const githubUrl = mp?.repo
            ? mp.repo.startsWith('http')
              ? mp.repo
              : `https://github.com/${mp.repo}`
            : null
          return (
            <PluginCard
              key={plugin.id}
              plugin={plugin}
              onAction={onAction}
              isPending={isPluginPending(plugin.name)}
              githubUrl={githubUrl}
            />
          )
        })}
      </div>
    </div>
  )
}
