import type { PluginInfo } from '../../types/generated'
import { PluginCard } from './PluginCard'
import { SectionHeader } from './SectionHeader'

interface InstalledPluginsSectionProps {
  plugins: PluginInfo[]
  onAction: (action: string, name: string, scope?: string, projectPath?: string | null) => void
  isPending: boolean
}

export function InstalledPluginsSection({
  plugins,
  onAction,
  isPending,
}: InstalledPluginsSectionProps) {
  return (
    <div>
      <SectionHeader title="Installed Plugins" count={plugins.length} />
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-2.5">
        {plugins.map((plugin) => (
          <PluginCard key={plugin.id} plugin={plugin} onAction={onAction} isPending={isPending} />
        ))}
      </div>
    </div>
  )
}
