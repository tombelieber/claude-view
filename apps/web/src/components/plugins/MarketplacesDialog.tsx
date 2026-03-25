import * as Dialog from '@radix-ui/react-dialog'
import { ExternalLink, Plus, RefreshCw, Settings, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { useMarketplaceRefresh } from '../../hooks/use-marketplace-refresh'
import { useMarketplaceMutations, useMarketplaces } from '../../hooks/use-marketplaces'
import { cn } from '../../lib/utils'
import { DialogContent, DialogOverlay } from '../ui/CenteredDialog'
import { marketplaceDotColor } from './marketplace-colors'

export function MarketplacesDialog() {
  const [open, setOpen] = useState(false)
  const [newSource, setNewSource] = useState('')
  const { data: marketplaces, isLoading } = useMarketplaces()
  const mutations = useMarketplaceMutations()
  const { refreshAll, isActive, getStatus, getError } = useMarketplaceRefresh()

  const handleAdd = () => {
    if (!newSource.trim()) return
    mutations.mutateAsync({ action: 'add', source: newSource.trim(), name: null, scope: null })
    setNewSource('')
  }

  const handleRemove = (name: string) => {
    mutations.mutateAsync({ action: 'remove', name, source: null, scope: null })
  }

  const handleRefreshAll = () => {
    refreshAll()
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 text-xs px-2 py-1 rounded-md border border-apple-sep text-apple-text2 hover:bg-apple-sep2 transition-colors"
          title="Manage marketplaces"
        >
          <Settings className="w-3.5 h-3.5" />
          Marketplaces
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <DialogOverlay />
        <DialogContent className="max-w-md rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-xl">
          <div className="flex items-center justify-between p-4 border-b border-gray-100 dark:border-gray-800">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-gray-100">
              Plugin Marketplaces
            </Dialog.Title>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleRefreshAll}
                disabled={isActive}
                className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors text-gray-500 dark:text-gray-400"
                title="Refresh all marketplaces"
              >
                <RefreshCw className={cn('w-4 h-4', isActive && 'animate-spin')} />
              </button>
              <Dialog.Close asChild>
                <button
                  type="button"
                  className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors text-gray-400"
                >
                  <X className="w-4 h-4" />
                </button>
              </Dialog.Close>
            </div>
          </div>

          <div className="p-4 max-h-80 overflow-y-auto">
            {isLoading && (
              <div className="text-xs text-gray-400 dark:text-gray-500 py-4 text-center">
                Loading...
              </div>
            )}

            {marketplaces && marketplaces.length === 0 && (
              <div className="text-xs text-gray-400 dark:text-gray-500 py-4 text-center">
                No marketplaces configured
              </div>
            )}

            {marketplaces?.map((m) => {
              const rowStatus = getStatus(m.name)
              const isRowBusy = rowStatus === 'queued' || rowStatus === 'running'

              return (
                <div
                  key={m.name}
                  className="py-2.5 border-b border-gray-50 dark:border-gray-800 last:border-0"
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2 min-w-0">
                      <span
                        className={cn(
                          'w-2 h-2 rounded-full flex-shrink-0',
                          (() => {
                            const status = getStatus(m.name)
                            if (status === 'queued')
                              return 'bg-gray-300 dark:bg-gray-600 animate-pulse'
                            if (status === 'running')
                              return cn(marketplaceDotColor(m.name), 'animate-ping')
                            if (status === 'failed') return 'bg-red-500'
                            return marketplaceDotColor(m.name)
                          })(),
                        )}
                      />
                      <div className="min-w-0">
                        <div className="text-xs font-medium text-gray-900 dark:text-gray-100 truncate">
                          {m.name}
                        </div>
                        <div className="text-xs text-gray-400 dark:text-gray-500 truncate">
                          {m.source}
                        </div>
                      </div>
                    </div>
                    <div className="flex items-center gap-1 flex-shrink-0">
                      {m.repo && (
                        <a
                          href={m.repo.startsWith('http') ? m.repo : `https://github.com/${m.repo}`}
                          target="_blank"
                          rel="noopener noreferrer"
                          className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 transition-colors"
                          title="Open repository"
                          onClick={(e) => e.stopPropagation()}
                        >
                          <ExternalLink className="w-3.5 h-3.5" />
                        </a>
                      )}
                      <button
                        type="button"
                        onClick={() => handleRemove(m.name)}
                        disabled={mutations.isPending || isRowBusy}
                        className="p-1 rounded hover:bg-red-50 dark:hover:bg-red-900/20 text-gray-400 hover:text-red-500 transition-colors"
                        title={`Remove ${m.name}`}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                  {/* Status / Counts row */}
                  <div className="flex items-center gap-3 mt-1 ml-4 text-xs">
                    {(() => {
                      const status = getStatus(m.name)
                      if (!status || status === 'completed') {
                        return (
                          <span className="text-gray-400 dark:text-gray-500">
                            {m.installedCount} installed · {m.availableCount} available
                          </span>
                        )
                      }
                      if (status === 'queued') {
                        return (
                          <output className="text-gray-400 dark:text-gray-500 animate-pulse">
                            Queued
                          </output>
                        )
                      }
                      if (status === 'running') {
                        return (
                          <output className="text-blue-500 dark:text-blue-400">Updating...</output>
                        )
                      }
                      if (status === 'failed') {
                        return (
                          <span className="flex items-center gap-1.5">
                            <output className="text-red-500">{getError(m.name) || 'Failed'}</output>
                            <button
                              type="button"
                              onClick={() => refreshAll([m.name])}
                              className="p-0.5 rounded hover:bg-red-50 dark:hover:bg-red-900/20 text-red-400 hover:text-red-600 transition-colors"
                              aria-label={`Retry updating ${m.name}`}
                            >
                              <RefreshCw className="w-3 h-3" />
                            </button>
                          </span>
                        )
                      }
                      return null
                    })()}
                  </div>
                </div>
              )
            })}
          </div>

          <div className="p-4 border-t border-gray-100 dark:border-gray-800">
            <div className="flex gap-2">
              <input
                type="text"
                value={newSource}
                onChange={(e) => setNewSource(e.target.value)}
                onKeyDown={(e) => e.key === 'Enter' && handleAdd()}
                placeholder="owner/repo or GitHub URL"
                className="flex-1 text-xs px-3 py-1.5 rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder-gray-400 dark:placeholder-gray-500 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />
              <button
                type="button"
                onClick={handleAdd}
                disabled={!newSource.trim() || mutations.isPending}
                className="flex items-center gap-1 text-xs px-3 py-1.5 rounded-md bg-blue-600 text-white hover:bg-blue-700 transition-colors disabled:opacity-50"
              >
                <Plus className="w-3.5 h-3.5" />
                Add
              </button>
            </div>
          </div>
        </DialogContent>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
