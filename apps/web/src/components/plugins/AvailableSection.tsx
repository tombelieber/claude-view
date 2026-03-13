import type { AvailablePlugin, MarketplaceInfo } from '../../types/generated'
import { AvailablePluginCard } from './AvailablePluginCard'
import { SectionHeader } from './SectionHeader'

interface AvailableSectionProps {
  plugins: AvailablePlugin[]
  onInstall: (name: string, scope: string) => void
  isPluginPending: (name: string) => boolean
  marketplaces?: MarketplaceInfo[]
}

export function AvailableSection({
  plugins,
  onInstall,
  isPluginPending,
  marketplaces = [],
}: AvailableSectionProps) {
  return (
    <div>
      <SectionHeader title="Available in Marketplaces" count={plugins.length} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {plugins.map((p) => {
          const mp = marketplaces.find((m) => m.name === p.marketplaceName)
          const githubUrl = mp?.repo
            ? mp.repo.startsWith('http')
              ? mp.repo
              : `https://github.com/${mp.repo}`
            : null
          return (
            <AvailablePluginCard
              key={p.pluginId}
              plugin={p}
              onInstall={onInstall}
              isPending={isPluginPending(p.name)}
              githubUrl={githubUrl}
            />
          )
        })}
      </div>
    </div>
  )
}
