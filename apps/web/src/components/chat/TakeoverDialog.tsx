import * as AlertDialog from '@radix-ui/react-alert-dialog'
// E-M4: Only AlertDialogContent from CenteredDialog (centering).
// Title, Description, Action, Cancel from Radix directly.
import { AlertDialogContent, AlertDialogOverlay } from '../ui/CenteredDialog'

interface TakeoverDialogProps {
  open: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function TakeoverDialog({ open, onConfirm, onCancel }: TakeoverDialogProps) {
  return (
    <AlertDialog.Root open={open} onOpenChange={(o) => !o && onCancel()}>
      <AlertDialog.Portal>
        <AlertDialogOverlay />
        <AlertDialogContent className="max-w-md rounded-lg bg-white dark:bg-gray-900 shadow-xl p-6">
          <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Take over this session?
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-sm text-gray-600 dark:text-gray-400">
            This will terminate the Claude CLI process currently running this session. The CLI's
            work will be saved. You'll continue in Claude View.
          </AlertDialog.Description>
          <div className="flex justify-end gap-2 mt-4">
            <AlertDialog.Cancel
              onClick={onCancel}
              className="px-3 py-1.5 text-sm rounded-md border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800"
            >
              Cancel
            </AlertDialog.Cancel>
            <AlertDialog.Action
              onClick={onConfirm}
              className="px-3 py-1.5 text-sm rounded-md bg-red-600 text-white hover:bg-red-700"
            >
              Take Over
            </AlertDialog.Action>
          </div>
        </AlertDialogContent>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
