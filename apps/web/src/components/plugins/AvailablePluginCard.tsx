import { Download } from 'lucide-react'
import { cn } from '../../lib/utils'
import type { AvailablePlugin } from '../../types/generated'
import { marketplaceDotColor } from './marketplace-colors'

interface AvailablePluginCardProps {
  plugin: AvailablePlugin
  onInstall: (name: string, scope: string) => void
  isPending: boolean
}

function formatInstallCount(count: bigint | null): string | null {
  if (count === null) return null
  const n = Number(count)
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toString()
}

export function AvailablePluginCard({ plugin, onInstall, isPending }: AvailablePluginCardProps) {
  const installs = formatInstallCount(plugin.installCount)

  return (
    <div
      className={cn(
        'rounded-lg border-2 border-dashed p-3 transition-colors duration-200',
        'border-gray-200 dark:border-gray-700',
        'bg-gray-50/50 dark:bg-gray-900/30',
        'hover:border-blue-300 dark:hover:border-blue-600',
      )}
    >
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold text-gray-900 dark:text-gray-100 truncate">
          {plugin.name}
        </h3>
        {plugin.alreadyInstalled ? (
          <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 flex-shrink-0">
            INSTALLED
          </span>
        ) : (
          <button
            type="button"
            disabled={isPending}
            onClick={() => onInstall(plugin.pluginId, 'user')}
            className={cn(
              'flex items-center gap-1 text-[10px] px-2 py-1 rounded font-medium transition-colors flex-shrink-0',
              'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400',
              'hover:bg-blue-100 dark:hover:bg-blue-900/50',
              'disabled:opacity-50 cursor-pointer',
            )}
          >
            <Download className="w-3 h-3" />
            GET
          </button>
        )}
      </div>

      {/* Marketplace + version + installs */}
      <div className="flex items-center gap-2 mt-1 text-[10px] text-gray-500 dark:text-gray-500">
        <span className="flex items-center gap-1">
          <span
            className={cn(
              'w-2 h-2 rounded-full inline-block',
              marketplaceDotColor(plugin.marketplaceName),
            )}
          />
          {plugin.marketplaceName}
        </span>
        {plugin.version && (
          <>
            <span className="text-gray-300 dark:text-gray-700">&middot;</span>
            <span className="font-mono">{plugin.version}</span>
          </>
        )}
        {installs && (
          <>
            <span className="text-gray-300 dark:text-gray-700">&middot;</span>
            <span>{installs} installs</span>
          </>
        )}
      </div>

      {/* Description */}
      <p className="text-xs text-gray-500 dark:text-gray-400 mt-2 line-clamp-2">
        {plugin.description}
      </p>
    </div>
  )
}
