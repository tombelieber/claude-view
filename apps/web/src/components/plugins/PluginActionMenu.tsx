import * as Popover from '@radix-ui/react-popover'
import { MoreHorizontal, RefreshCw, ToggleLeft, ToggleRight, Trash2 } from 'lucide-react'
import { useState } from 'react'
import type { PluginInfo } from '../../types/generated'
import { UninstallConfirmDialog } from './UninstallConfirmDialog'

interface PluginActionMenuProps {
  plugin: PluginInfo
  onAction: (action: string, name: string, scope?: string) => void
  isPending: boolean
}

export function PluginActionMenu({ plugin, onAction, isPending }: PluginActionMenuProps) {
  const [open, setOpen] = useState(false)
  const [confirmUninstall, setConfirmUninstall] = useState(false)

  const handleAction = (action: string) => {
    setOpen(false)
    if (action === 'uninstall') {
      setConfirmUninstall(true)
      return
    }
    onAction(action, plugin.name, plugin.scope)
  }

  return (
    <>
      <Popover.Root open={open} onOpenChange={setOpen}>
        <Popover.Trigger asChild>
          <button
            type="button"
            onClick={(e) => e.stopPropagation()}
            className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            title="Plugin actions"
            disabled={isPending}
          >
            <MoreHorizontal className="w-4 h-4 text-gray-400" />
          </button>
        </Popover.Trigger>
        <Popover.Portal>
          <Popover.Content
            align="end"
            sideOffset={4}
            className="z-50 w-44 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg py-1"
            onClick={(e) => e.stopPropagation()}
          >
            {plugin.updatable && (
              <MenuItem
                icon={<RefreshCw className="w-3.5 h-3.5" />}
                label="Update"
                onClick={() => handleAction('update')}
              />
            )}
            <MenuItem
              icon={
                plugin.enabled ? (
                  <ToggleLeft className="w-3.5 h-3.5" />
                ) : (
                  <ToggleRight className="w-3.5 h-3.5" />
                )
              }
              label={plugin.enabled ? 'Disable' : 'Enable'}
              onClick={() => handleAction(plugin.enabled ? 'disable' : 'enable')}
            />
            <div className="my-1 border-t border-gray-100 dark:border-gray-800" />
            <MenuItem
              icon={<Trash2 className="w-3.5 h-3.5" />}
              label="Uninstall"
              onClick={() => handleAction('uninstall')}
              destructive
            />
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>

      <UninstallConfirmDialog
        plugin={plugin}
        open={confirmUninstall}
        onOpenChange={setConfirmUninstall}
        onConfirm={() => {
          setConfirmUninstall(false)
          onAction('uninstall', plugin.name, plugin.scope)
        }}
      />
    </>
  )
}

function MenuItem({
  icon,
  label,
  onClick,
  destructive,
}: {
  icon: React.ReactNode
  label: string
  onClick: () => void
  destructive?: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors ${
        destructive
          ? 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20'
          : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800'
      }`}
    >
      {icon}
      {label}
    </button>
  )
}
