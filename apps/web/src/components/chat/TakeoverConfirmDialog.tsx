import * as AlertDialog from '@radix-ui/react-alert-dialog'
import { AlertTriangle } from 'lucide-react'
import { useCallback, useRef } from 'react'
import { AlertDialogContent, AlertDialogOverlay } from '../ui/CenteredDialog'

interface TakeoverConfirmDialogProps {
  open: boolean
  onConfirm: () => void
  onCancel: () => void
}

export function TakeoverConfirmDialog({ open, onConfirm, onCancel }: TakeoverConfirmDialogProps) {
  const checkboxRef = useRef<HTMLInputElement>(null)

  const handleConfirm = useCallback(() => {
    if (checkboxRef.current?.checked) {
      localStorage.setItem('claude-view:takeover-no-remind', 'true')
    }
    onConfirm()
  }, [onConfirm])

  return (
    <AlertDialog.Root open={open} onOpenChange={(v) => !v && onCancel()}>
      <AlertDialog.Portal>
        <AlertDialogOverlay className="bg-black/50 dark:bg-black/70" />
        <AlertDialogContent className="max-w-md bg-white dark:bg-gray-900 rounded-xl shadow-xl p-6">
          <AlertDialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <AlertTriangle className="h-5 w-5 text-blue-500" />
            Continue in Claude View?
          </AlertDialog.Title>

          <AlertDialog.Description className="text-sm text-gray-600 dark:text-gray-400 mt-3 leading-relaxed">
            This session was started outside claude-view (CLI / VS Code). Forking will create a copy
            of the conversation here so you can continue it in this panel. The original CLI session
            will keep running separately.
          </AlertDialog.Description>

          <label className="mt-4 flex items-center gap-2 cursor-pointer select-none">
            <input
              ref={checkboxRef}
              type="checkbox"
              className="w-4 h-4 rounded border-gray-300 dark:border-gray-600 text-amber-600 focus:ring-amber-500"
            />
            <span className="text-xs text-gray-500 dark:text-gray-400">
              Don&apos;t remind me again
            </span>
          </label>

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
                onClick={handleConfirm}
                className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
              >
                Fork &amp; Continue
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialogContent>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}
