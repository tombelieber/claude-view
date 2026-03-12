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
    <div className="rounded-xl border border-dashed border-apple-sep bg-apple-bg p-4 hover:border-apple-blue transition-colors duration-150">
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <h3 className="text-sm font-semibold text-apple-text1 truncate">{plugin.name}</h3>
        {plugin.alreadyInstalled ? (
          <span className="text-[10px] font-bold uppercase tracking-[0.05em] px-1.5 py-0.5 rounded-[5px] bg-[rgba(52,199,89,0.1)] text-[#248A3D] border border-[rgba(52,199,89,0.2)] flex-shrink-0">
            INSTALLED
          </span>
        ) : (
          <button
            type="button"
            disabled={isPending}
            onClick={() => onInstall(plugin.name, 'user')}
            className={cn(
              'inline-flex items-center gap-1 text-[11px] font-semibold px-2.5 py-1 rounded-[7px]',
              'bg-[rgba(0,122,255,0.1)] text-apple-blue border border-[rgba(0,122,255,0.18)]',
              'hover:bg-[rgba(0,122,255,0.18)] transition-colors',
              'disabled:opacity-50 cursor-pointer flex-shrink-0',
            )}
          >
            GET
          </button>
        )}
      </div>

      {/* Marketplace + version + installs */}
      <div className="flex items-center gap-2 mt-1 text-[10px] text-apple-text3">
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
            <span className="text-apple-sep">&middot;</span>
            <span className="font-mono">{plugin.version}</span>
          </>
        )}
        {installs && (
          <>
            <span className="text-apple-sep">&middot;</span>
            <span>{installs} installs</span>
          </>
        )}
      </div>

      {/* Description */}
      <p className="text-[13px] text-apple-text2 mt-1.5 line-clamp-2">{plugin.description}</p>
    </div>
  )
}
