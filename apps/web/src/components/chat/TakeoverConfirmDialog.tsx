import * as AlertDialog from '@radix-ui/react-alert-dialog'
import { AlertTriangle } from 'lucide-react'

interface TakeoverConfirmDialogProps {
  open: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function TakeoverConfirmDialog({ open, onConfirm, onCancel }: TakeoverConfirmDialogProps) {
  return (
    <AlertDialog.Root open={open} onOpenChange={(v) => !v && onCancel()}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 bg-black/50 dark:bg-black/70" />
        <AlertDialog.Content className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-md bg-white dark:bg-gray-900 rounded-xl shadow-xl p-6 focus:outline-none">
          <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-amber-500" />
            Take Control?
          </AlertDialog.Title>

          <AlertDialog.Description className="text-sm text-gray-600 dark:text-gray-400 mt-3 leading-relaxed">
            This session was started outside claude-view (CLI / VS Code). Taking control will
            disconnect any active terminal input and route all interaction through this panel. The
            session&apos;s work will continue unaffected.
          </AlertDialog.Description>

          <div className="mt-6 flex justify-end gap-3">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700"
              >
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="px-4 py-2 text-sm font-medium text-white bg-amber-600 rounded-md hover:bg-amber-700"
              >
                Take Control
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
