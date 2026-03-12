import type { PermissionRequest } from '@claude-view/shared/types/sidecar-protocol'
import { ShieldAlert } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '../../../../lib/utils'
import { InteractiveCardShell } from '../../../chat/cards/InteractiveCardShell'

function getToolDisplay(
  toolName: string,
  toolInput: Record<string, unknown>,
): { label: string; content: string } {
  switch (toolName) {
    case 'Bash':
      return { label: 'Command', content: String(toolInput.command ?? '') }
    case 'Edit':
    case 'Write':
      return {
        label: `File: ${String(toolInput.file_path ?? toolInput.filePath ?? 'unknown')}`,
        content: toolInput.new_string
          ? `Replace:\n${String(toolInput.old_string ?? '')}\n\nWith:\n${String(toolInput.new_string)}`
          : String(toolInput.content ?? JSON.stringify(toolInput, null, 2)),
      }
    case 'Read':
      return { label: 'File', content: String(toolInput.file_path ?? toolInput.filePath ?? '') }
    default:
      return { label: toolName, content: JSON.stringify(toolInput, null, 2) }
  }
}

export interface PermissionCardProps {
  permission: PermissionRequest
  onRespond?: (requestId: string, allowed: boolean) => void
  resolved?: { allowed: boolean }
}

export function PermissionCard({ permission, onRespond, resolved }: PermissionCardProps) {
  const totalSeconds = Math.ceil(permission.timeoutMs / 1000)
  const [countdown, setCountdown] = useState(totalSeconds)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const onRespondRef = useRef(onRespond)
  onRespondRef.current = onRespond

  const requestId = permission.requestId
  const timeoutMs = permission.timeoutMs

  useEffect(() => {
    if (resolved || !onRespondRef.current) return

    const secs = Math.ceil(timeoutMs / 1000)
    setCountdown(secs)

    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          onRespondRef.current?.(requestId, false)
          return 0
        }
        return prev - 1
      })
    }, 1000)

    return () => {
      if (timerRef.current) clearInterval(timerRef.current)
    }
  }, [requestId, timeoutMs, resolved])

  const handleAllow = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    onRespond?.(requestId, true)
  }, [onRespond, requestId])

  const handleDeny = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    onRespond?.(requestId, false)
  }, [onRespond, requestId])

  const toolDisplay = getToolDisplay(permission.toolName, permission.toolInput)

  const resolvedState = resolved
    ? resolved.allowed
      ? { label: 'Allowed', variant: 'success' as const }
      : { label: 'Denied', variant: 'denied' as const }
    : undefined

  return (
    <InteractiveCardShell
      variant="permission"
      header="Permission Required"
      icon={<ShieldAlert className="w-4 h-4" />}
      resolved={resolvedState}
      actions={
        onRespond ? (
          <>
            <button
              type="button"
              onClick={handleDeny}
              className="px-3 py-1.5 text-xs font-medium text-red-700 dark:text-red-400 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800/40 rounded-md hover:bg-red-100 dark:hover:bg-red-900/30 transition-colors"
            >
              Deny
            </button>
            <button
              type="button"
              onClick={handleAllow}
              className="px-3 py-1.5 text-xs font-medium text-white bg-green-600 rounded-md hover:bg-green-700 transition-colors"
            >
              Allow
            </button>
          </>
        ) : undefined
      }
    >
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <span className="inline-flex items-center px-2 py-0.5 text-[11px] font-mono font-medium bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded">
            {permission.toolName}
          </span>
        </div>

        {permission.decisionReason && (
          <p className="text-xs text-gray-700 dark:text-gray-300">{permission.decisionReason}</p>
        )}

        <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
          {toolDisplay.label && (
            <div className="px-2 py-1 text-[10px] font-medium text-gray-500 dark:text-gray-400 border-b border-gray-200/50 dark:border-gray-700/50 bg-gray-50 dark:bg-gray-800/30">
              {toolDisplay.label}
            </div>
          )}
          <pre className="px-2 py-1.5 text-[11px] text-gray-800 dark:text-gray-200 overflow-x-auto max-h-32 whitespace-pre-wrap font-mono">
            {toolDisplay.content}
          </pre>
        </div>

        {!resolved && (
          <div className="flex items-center gap-2">
            <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className={cn(
                  'h-full rounded-full transition-all duration-1000 ease-linear',
                  countdown < 10 ? 'bg-red-500 animate-pulse' : 'bg-amber-500',
                )}
                style={{ width: `${(countdown / totalSeconds) * 100}%` }}
              />
            </div>
            <span
              className={cn(
                'text-[10px] font-mono tabular-nums w-6 text-right',
                countdown < 10
                  ? 'text-red-500 dark:text-red-400 font-bold'
                  : 'text-gray-500 dark:text-gray-400',
              )}
            >
              {countdown}s
            </span>
          </div>
        )}
      </div>
    </InteractiveCardShell>
  )
}
