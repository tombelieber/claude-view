import * as AlertDialog from '@radix-ui/react-alert-dialog'
import type { PluginInfo } from '../../types/generated'

interface UninstallConfirmDialogProps {
  plugin: PluginInfo
  open: boolean
  onOpenChange: (open: boolean) => void
  onConfirm: () => void
}

export function UninstallConfirmDialog({
  plugin,
  open,
  onOpenChange,
  onConfirm,
}: UninstallConfirmDialogProps) {
  const parts: string[] = []
  if (plugin.skillCount > 0)
    parts.push(`${plugin.skillCount} skill${plugin.skillCount > 1 ? 's' : ''}`)
  if (plugin.agentCount > 0)
    parts.push(`${plugin.agentCount} agent${plugin.agentCount > 1 ? 's' : ''}`)
  if (plugin.commandCount > 0)
    parts.push(`${plugin.commandCount} command${plugin.commandCount > 1 ? 's' : ''}`)
  if (plugin.mcpCount > 0)
    parts.push(`${plugin.mcpCount} MCP tool${plugin.mcpCount > 1 ? 's' : ''}`)

  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <AlertDialog.Content
          className="fixed z-[51] top-1/2 left-1/2 w-full max-w-sm rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-6 shadow-xl"
          style={{ transform: 'translate(-50%, -50%)' }}
        >
          <AlertDialog.Title className="text-sm font-semibold text-gray-900 dark:text-gray-100">
            Uninstall {plugin.name}?
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-xs text-gray-500 dark:text-gray-400">
            {parts.length > 0
              ? `This will remove ${parts.join(', ')} from your Claude Code setup.`
              : 'This will remove the plugin from your Claude Code setup.'}
          </AlertDialog.Description>
          <div className="mt-4 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="px-3 py-1.5 text-xs rounded-md border border-gray-200 dark:border-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800"
              >
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="px-3 py-1.5 text-xs rounded-md bg-red-600 text-white hover:bg-red-700"
              >
                Uninstall
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
