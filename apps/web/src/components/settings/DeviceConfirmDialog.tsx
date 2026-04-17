import * as AlertDialog from '@radix-ui/react-alert-dialog'
import { AlertDialogContent, AlertDialogOverlay } from '../ui/CenteredDialog'

interface DeviceConfirmDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  title: string
  description: string
  confirmLabel: string
  onConfirm: () => void
}

/**
 * Destructive-action confirmation dialog used for revoke and
 * sign-out-others. Matches the inline-style centering mandate via the
 * shared {@link AlertDialogContent} primitive.
 */
export function DeviceConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  onConfirm,
}: DeviceConfirmDialogProps) {
  return (
    <AlertDialog.Root open={open} onOpenChange={onOpenChange}>
      <AlertDialog.Portal>
        <AlertDialogOverlay className="bg-black/50" />
        <AlertDialogContent className="bg-white dark:bg-gray-900 rounded-lg max-w-sm shadow-xl p-6">
          <AlertDialog.Title className="text-base font-semibold text-gray-900 dark:text-gray-100">
            {title}
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-sm text-gray-500 dark:text-gray-400">
            {description}
          </AlertDialog.Description>
          <div className="mt-5 flex items-center justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800"
              >
                Cancel
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="px-3 py-1.5 text-sm font-medium text-white bg-red-600 rounded-md hover:bg-red-700"
              >
                {confirmLabel}
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialogContent>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
