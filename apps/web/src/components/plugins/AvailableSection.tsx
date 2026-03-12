import type { AvailablePlugin } from '../../types/generated'
import { AvailablePluginCard } from './AvailablePluginCard'
import { SectionHeader } from './SectionHeader'

interface AvailableSectionProps {
  plugins: AvailablePlugin[]
  onInstall: (name: string, scope: string) => void
  isPending: boolean
}

export function AvailableSection({ plugins, onInstall, isPending }: AvailableSectionProps) {
  return (
    <div>
      <SectionHeader title="Available in Marketplaces" count={plugins.length} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {plugins.map((p) => (
          <AvailablePluginCard
            key={p.pluginId}
            plugin={p}
            onInstall={onInstall}
            isPending={isPending}
          />
        ))}
      </div>
    </div>
  )
}
