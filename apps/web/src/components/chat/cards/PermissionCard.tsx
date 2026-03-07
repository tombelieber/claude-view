import { ShieldAlert } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { InteractiveCardShell } from './InteractiveCardShell'

export interface PermissionCardProps {
  permission: {
    requestId: string
    toolName: string
    toolInput: Record<string, unknown>
    description: string
    timeoutMs: number
  }
  onRespond: (requestId: string, allowed: boolean) => void
  resolved?: { allowed: boolean }
}

/**
 * Tool-specific display info. Re-implemented inline to avoid coupling to
 * PermissionDialog which lives in a different directory.
 */
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

export function PermissionCard({ permission, onRespond, resolved }: PermissionCardProps) {
  const totalSeconds = Math.ceil(permission.timeoutMs / 1000)
  const [countdown, setCountdown] = useState(totalSeconds)
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null)
  const onRespondRef = useRef(onRespond)
  onRespondRef.current = onRespond

  // Derive primitives for stable deps (per CLAUDE.md: useMemo on a primitive key)
  const requestId = permission.requestId
  const timeoutMs = permission.timeoutMs

  useEffect(() => {
    if (resolved) return

    const secs = Math.ceil(timeoutMs / 1000)
    setCountdown(secs)

    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          onRespondRef.current(requestId, false)
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
    onRespond(requestId, true)
  }, [onRespond, requestId])

  const handleDeny = useCallback(() => {
    if (timerRef.current) clearInterval(timerRef.current)
    onRespond(requestId, false)
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
      }
    >
      <div className="space-y-2">
        {/* Tool name badge */}
        <div className="flex items-center gap-2">
          <span className="inline-flex items-center px-2 py-0.5 text-[11px] font-mono font-medium bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded">
            {permission.toolName}
          </span>
        </div>

        {/* Description */}
        {permission.description && (
          <p className="text-xs text-gray-700 dark:text-gray-300">{permission.description}</p>
        )}

        {/* Tool-specific preview */}
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

        {/* Countdown bar */}
        {!resolved && (
          <div className="flex items-center gap-2">
            <div className="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                className="h-full bg-amber-500 rounded-full transition-all duration-1000 ease-linear"
                style={{
                  width: `${(countdown / totalSeconds) * 100}%`,
                }}
              />
            </div>
            <span className="text-[10px] font-mono text-gray-500 dark:text-gray-400 tabular-nums w-6 text-right">
              {countdown}s
            </span>
          </div>
        )}
      </div>
    </InteractiveCardShell>
  )
}
