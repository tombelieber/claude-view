import * as Dialog from '@radix-ui/react-dialog'
import { useEffect, useRef, useState } from 'react'
import type { PermissionRequestMsg } from '../../types/control'

interface PermissionDialogProps {
  request: PermissionRequestMsg | null
  onRespond: (requestId: string, allowed: boolean) => void
}

export function PermissionDialog({ request, onRespond }: PermissionDialogProps) {
  const [countdown, setCountdown] = useState(60)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)

  // Reset countdown when a new request comes in
  useEffect(() => {
    if (!request) return

    const totalSeconds = Math.ceil((request.timeoutMs || 60000) / 1000)
    setCountdown(totalSeconds)

    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          // Auto-deny on timeout
          onRespond(request.requestId, false)
          return 0
        }
        return prev - 1
      })
    }, 1000)

    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [request?.requestId]) // Only re-run when requestId changes

  if (!request) return null

  const handleAllow = () => {
    if (timerRef.current) clearInterval(timerRef.current)
    onRespond(request.requestId, true)
  }

  const handleDeny = () => {
    if (timerRef.current) clearInterval(timerRef.current)
    onRespond(request.requestId, false)
  }

  // Extract display info based on tool type
  const toolDisplay = getToolDisplay(request.toolName, request.toolInput)

  return (
    <Dialog.Root open={!!request}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 dark:bg-black/70" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-lg bg-white dark:bg-gray-900 rounded-xl shadow-xl p-6 focus:outline-none"
          onEscapeKeyDown={(e) => e.preventDefault()}
          onPointerDownOutside={(e) => e.preventDefault()}
        >
          <Dialog.Title className="text-lg font-semibold text-gray-900 dark:text-gray-100 flex items-center gap-2">
            <span className="text-amber-500">&#x26A0;</span>
            Permission Required
          </Dialog.Title>

          <Dialog.Description className="text-sm text-gray-500 dark:text-gray-400 mt-1">
            Claude wants to use a tool. Review and approve or deny.
          </Dialog.Description>

          {/* Tool Info */}
          <div className="mt-4 space-y-3">
            {/* Tool name badge */}
            <div className="flex items-center gap-2">
              <span className="inline-flex items-center px-2 py-1 text-xs font-mono font-medium bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded">
                {request.toolName}
              </span>
            </div>

            {/* Description */}
            {request.description && (
              <p className="text-sm text-gray-700 dark:text-gray-300">{request.description}</p>
            )}

            {/* Tool-specific preview */}
            <div className="rounded-lg bg-gray-50 dark:bg-gray-800/50 border border-gray-200 dark:border-gray-700 overflow-hidden">
              {toolDisplay.label && (
                <div className="px-3 py-1.5 text-xs font-medium text-gray-500 dark:text-gray-400 border-b border-gray-200 dark:border-gray-700">
                  {toolDisplay.label}
                </div>
              )}
              <pre className="px-3 py-2 text-xs text-gray-800 dark:text-gray-200 overflow-x-auto max-h-48 whitespace-pre-wrap font-mono">
                {toolDisplay.content}
              </pre>
            </div>

            {/* Countdown */}
            <div className="flex items-center gap-2">
              <div className="flex-1 h-1.5 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
                <div
                  className="h-full bg-amber-500 rounded-full transition-all duration-1000 ease-linear"
                  style={{
                    width: `${(countdown / Math.ceil((request.timeoutMs || 60000) / 1000)) * 100}%`,
                  }}
                />
              </div>
              <span className="text-xs font-mono text-gray-500 dark:text-gray-400 tabular-nums w-8 text-right">
                {countdown}s
              </span>
            </div>
          </div>

          {/* Actions */}
          <div className="mt-6 flex justify-end gap-3">
            <button
              type="button"
              onClick={handleDeny}
              className="px-4 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700"
            >
              Deny
            </button>
            <button
              type="button"
              onClick={handleAllow}
              className="px-4 py-2 text-sm font-medium text-white bg-blue-600 rounded-md hover:bg-blue-700"
            >
              Allow
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}

function getToolDisplay(
  toolName: string,
  toolInput: Record<string, unknown>,
): { label: string; content: string } {
  switch (toolName) {
    case 'Bash':
      return {
        label: 'Command',
        content: String(toolInput.command ?? ''),
      }
    case 'Edit':
    case 'Write':
      return {
        label: `File: ${String(toolInput.file_path ?? toolInput.filePath ?? 'unknown')}`,
        content: toolInput.new_string
          ? `Replace:\n${String(toolInput.old_string ?? '')}\n\nWith:\n${String(toolInput.new_string)}`
          : String(toolInput.content ?? JSON.stringify(toolInput, null, 2)),
      }
    case 'Read':
      return {
        label: 'File',
        content: String(toolInput.file_path ?? toolInput.filePath ?? ''),
      }
    default:
      return {
        label: toolName,
        content: JSON.stringify(toolInput, null, 2),
      }
  }
}
