import type { PermissionRequest } from '../../../../types/sidecar-protocol'
import { ShieldAlert } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { InteractiveCardShell } from './InteractiveCardShell'

/** Human-friendly "time waiting for your decision" label. */
function formatWaiting(seconds: number): string {
  if (seconds < 60) return `${seconds}s`
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`
}

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
  onAlwaysAllow?: (requestId: string, allowed: boolean, updatedPermissions: unknown[]) => void
  resolved?: { allowed: boolean }
}

export function PermissionCard({
  permission,
  onRespond,
  onAlwaysAllow,
  resolved,
}: PermissionCardProps) {
  const requestId = permission.requestId
  const interactive = !!onRespond

  // Trust-over-accuracy: a permission prompt NEVER auto-resolves. We show how
  // long it has been awaiting your decision (an attention cue), pausing the
  // counter while the tab is hidden so it reflects actual attention — but the
  // prompt stays pending until you decide. 寧願唔顯示，都唔顯示錯嘅嘢.
  const [waiting, setWaiting] = useState(0)
  useEffect(() => {
    if (resolved || !interactive) return
    setWaiting(0)
    let id: ReturnType<typeof setInterval> | null = null
    const start = () => {
      if (id == null) id = setInterval(() => setWaiting((w) => w + 1), 1000)
    }
    const stop = () => {
      if (id != null) {
        clearInterval(id)
        id = null
      }
    }
    const onVisibility = () => (document.visibilityState === 'hidden' ? stop() : start())
    if (document.visibilityState !== 'hidden') start()
    document.addEventListener('visibilitychange', onVisibility)
    return () => {
      stop()
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [requestId, resolved, interactive])

  const handleAllow = useCallback(() => {
    onRespond?.(requestId, true)
  }, [onRespond, requestId])

  const handleDeny = useCallback(() => {
    onRespond?.(requestId, false)
  }, [onRespond, requestId])

  const handleAlwaysAllow = useCallback(() => {
    if (permission.suggestions && onAlwaysAllow) {
      onAlwaysAllow(requestId, true, permission.suggestions)
    }
  }, [onAlwaysAllow, requestId, permission.suggestions])

  const toolDisplay = getToolDisplay(permission.toolName, permission.toolInput)

  const resolvedState = resolved
    ? resolved.allowed
      ? { label: 'Allowed', variant: 'success' as const }
      : { label: 'Denied', variant: 'denied' as const }
    : undefined

  const hasSuggestions = permission.suggestions && permission.suggestions.length > 0

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
            {hasSuggestions && onAlwaysAllow && (
              <button
                type="button"
                onClick={handleAlwaysAllow}
                className="px-3 py-1.5 text-xs font-medium text-blue-700 dark:text-blue-400 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800/40 rounded-md hover:bg-blue-100 dark:hover:bg-blue-900/30 transition-colors"
              >
                Always Allow
              </button>
            )}
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
          <span className="inline-flex items-center px-2 py-0.5 text-xs font-mono font-medium bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 rounded">
            {permission.toolName}
          </span>
        </div>

        {permission.decisionReason && (
          <p className="text-xs text-gray-700 dark:text-gray-300">{permission.decisionReason}</p>
        )}

        <div className="rounded border border-gray-200/50 dark:border-gray-700/50 overflow-hidden">
          {toolDisplay.label && (
            <div className="px-2 py-1 text-xs font-medium text-gray-500 dark:text-gray-400 border-b border-gray-200/50 dark:border-gray-700/50 bg-gray-50 dark:bg-gray-800/30">
              {toolDisplay.label}
            </div>
          )}
          <pre className="px-2 py-1.5 text-xs text-gray-800 dark:text-gray-200 overflow-x-auto max-h-32 whitespace-pre-wrap font-mono">
            {toolDisplay.content}
          </pre>
        </div>

        <span className="text-xs font-mono text-gray-400 dark:text-gray-500">
          ID: {permission.toolUseID}
        </span>

        {permission.blockedPath && (
          <div className="text-xs text-red-600 dark:text-red-400">
            Blocked: {permission.blockedPath}
          </div>
        )}

        {permission.agentID && (
          <div className="text-xs text-indigo-600 dark:text-indigo-400">
            Agent: {permission.agentID}
          </div>
        )}

        {!resolved && interactive && (
          <div className="flex items-center gap-2 text-xs text-amber-600 dark:text-amber-400">
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
            <span className="tabular-nums">
              Waiting for your response{waiting > 0 ? ` · ${formatWaiting(waiting)}` : ''}
            </span>
          </div>
        )}
      </div>
    </InteractiveCardShell>
  )
}
