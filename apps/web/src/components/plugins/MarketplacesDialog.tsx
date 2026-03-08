import * as Dialog from '@radix-ui/react-dialog'
import { Plus, RefreshCw, Settings, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { useMarketplaceMutations, useMarketplaces } from '../../hooks/use-marketplaces'
import { cn } from '../../lib/utils'
import { marketplaceDotColor } from './marketplace-colors'

export function MarketplacesDialog() {
  const [open, setOpen] = useState(false)
  const [newSource, setNewSource] = useState('')
  const { data: marketplaces, isLoading } = useMarketplaces()
  const mutations = useMarketplaceMutations()

  const handleAdd = () => {
    if (!newSource.trim()) return
    mutations.mutateAsync({ action: 'add', source: newSource.trim(), name: null, scope: null })
    setNewSource('')
  }

  const handleRemove = (name: string) => {
    mutations.mutateAsync({ action: 'remove', name, source: null, scope: null })
  }

  const handleRefreshAll = () => {
    mutations.mutateAsync({ action: 'update', source: null, name: null, scope: null })
  }

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1 text-xs px-2 py-1 rounded-md text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          title="Manage marketplaces"
        >
          <Settings className="w-3.5 h-3.5" />
          Marketplaces
        </button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <Dialog.Content className="fixed z-50 top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-md rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-xl">
          <div className="flex items-center justify-between p-4 border-b border-gray-100 dark:border-gray-800">
            <Dialog.Title className="text-sm font-semibold text-gray-900 dark:text-gray-100">
              Plugin Marketplaces
            </Dialog.Title>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleRefreshAll}
                disabled={mutations.isPending}
                className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors text-gray-500 dark:text-gray-400"
                title="Refresh all marketplaces"
              >
                <RefreshCw className={cn('w-4 h-4', mutations.isPending && 'animate-spin')} />
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

            {marketplaces?.map((m) => (
              <div
                key={m.name}
                className="flex items-center justify-between py-2 border-b border-gray-50 dark:border-gray-800 last:border-0"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span
                    className={cn(
                      'w-2 h-2 rounded-full flex-shrink-0',
                      marketplaceDotColor(m.name),
                    )}
                  />
                  <div className="min-w-0">
                    <div className="text-xs font-medium text-gray-900 dark:text-gray-100 truncate">
                      {m.name}
                    </div>
                    <div className="text-[10px] text-gray-400 dark:text-gray-500 truncate">
                      {m.source}
                    </div>
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => handleRemove(m.name)}
                  disabled={mutations.isPending}
                  className="p-1 rounded hover:bg-red-50 dark:hover:bg-red-900/20 text-gray-400 hover:text-red-500 transition-colors flex-shrink-0"
                  title={`Remove ${m.name}`}
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            ))}
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
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
