import * as Popover from '@radix-ui/react-popover'
import { Download } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'
import type { AvailablePlugin } from '../../types/generated'
import { marketplaceDotColor } from './marketplace-colors'

interface AvailablePluginCardProps {
  plugin: AvailablePlugin
  onInstall: (name: string, scope: string) => void
  isPending: boolean
}

export function AvailablePluginCard({ plugin, onInstall, isPending }: AvailablePluginCardProps) {
  const [scopeOpen, setScopeOpen] = useState(false)

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
        <div className="flex items-center gap-2 flex-shrink-0">
          <span className="text-[10px] font-mono text-gray-400 dark:text-gray-500">
            {plugin.version}
          </span>
          {plugin.alreadyInstalled ? (
            <span className="text-[10px] px-1.5 py-0.5 rounded font-medium bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400">
              INSTALLED
            </span>
          ) : (
            <Popover.Root open={scopeOpen} onOpenChange={setScopeOpen}>
              <Popover.Trigger asChild>
                <button
                  type="button"
                  disabled={isPending}
                  className={cn(
                    'flex items-center gap-1 text-[10px] px-2 py-1 rounded font-medium transition-colors',
                    'bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400',
                    'hover:bg-blue-100 dark:hover:bg-blue-900/50',
                    'disabled:opacity-50',
                  )}
                >
                  <Download className="w-3 h-3" />
                  GET
                </button>
              </Popover.Trigger>
              <Popover.Portal>
                <Popover.Content
                  align="end"
                  sideOffset={4}
                  className="z-50 w-36 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg p-2"
                >
                  <div className="text-[10px] font-medium text-gray-500 dark:text-gray-400 mb-1 px-1">
                    Install scope
                  </div>
                  <ScopeOption
                    label="User"
                    description="All projects"
                    onClick={() => {
                      setScopeOpen(false)
                      onInstall(plugin.pluginId, 'user')
                    }}
                  />
                  <ScopeOption
                    label="Project"
                    description="This project only"
                    onClick={() => {
                      setScopeOpen(false)
                      onInstall(plugin.pluginId, 'project')
                    }}
                  />
                </Popover.Content>
              </Popover.Portal>
            </Popover.Root>
          )}
        </div>
      </div>

      {/* Marketplace */}
      <div className="flex items-center gap-1 mt-1">
        <span className="flex items-center gap-1 text-[10px] text-gray-500 dark:text-gray-500">
          <span
            className={cn(
              'w-2 h-2 rounded-full inline-block',
              marketplaceDotColor(plugin.marketplaceName),
            )}
          />
          {plugin.marketplaceName}
        </span>
      </div>

      {/* Description */}
      <p className="text-xs text-gray-500 dark:text-gray-400 mt-2 line-clamp-2">
        {plugin.description}
      </p>
    </div>
  )
}

function ScopeOption({
  label,
  description,
  onClick,
}: {
  label: string
  description: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="w-full flex flex-col px-2 py-1.5 rounded text-left hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
    >
      <span className="text-xs font-medium text-gray-900 dark:text-gray-100">{label}</span>
      <span className="text-[10px] text-gray-400 dark:text-gray-500">{description}</span>
    </button>
  )
}
