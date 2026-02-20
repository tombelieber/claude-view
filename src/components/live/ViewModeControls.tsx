import { useMonitorStore } from '../../store/monitor-store'
import { cn } from '../../lib/utils'

interface ViewModeControlsProps {
  className?: string
}

/**
 * Unified Chat / Debug + Rich / JSON segmented controls.
 *
 * Used in SessionDetailPanel (tab bar), TerminalOverlay (header),
 * and ConversationView (header). Single source of truth for the
 * view-mode toggle pattern.
 *
 * - Chat  = filtered conversation (user, assistant, error only)
 * - Debug = full execution trace (tool_use, tool_result, thinking, hooks)
 * - Rich  = formatted renderers for tool inputs (Debug only)
 * - JSON  = raw syntax-highlighted JSON (Debug only)
 */
export function ViewModeControls({ className }: ViewModeControlsProps) {
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const toggleVerbose = useMonitorStore((s) => s.toggleVerbose)
  const richRenderMode = useMonitorStore((s) => s.richRenderMode)
  const setRichRenderMode = useMonitorStore((s) => s.setRichRenderMode)

  return (
    <div className={cn('flex items-center gap-1', className)}>
      {/* Chat / Debug segmented control */}
      <div className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
        <button
          onClick={() => verboseMode && toggleVerbose()}
          className={cn(
            'text-[10px] font-medium px-2 py-1 transition-colors cursor-pointer',
            !verboseMode
              ? 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
              : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
          )}
        >
          Chat
        </button>
        <button
          onClick={() => !verboseMode && toggleVerbose()}
          className={cn(
            'text-[10px] font-medium px-2 py-1 transition-colors cursor-pointer',
            verboseMode
              ? 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
              : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
          )}
        >
          Debug
        </button>
      </div>

      {/* Rich / JSON toggle — only visible in Debug mode */}
      {verboseMode && (
        <div className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
          <button
            onClick={() => richRenderMode !== 'rich' && setRichRenderMode('rich')}
            className={cn(
              'text-[10px] font-medium px-2 py-1 transition-colors cursor-pointer',
              richRenderMode === 'rich'
                ? 'bg-emerald-50 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-400'
                : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
            )}
          >
            Rich
          </button>
          <button
            onClick={() => richRenderMode !== 'json' && setRichRenderMode('json')}
            className={cn(
              'text-[10px] font-medium px-2 py-1 transition-colors cursor-pointer',
              richRenderMode === 'json'
                ? 'bg-amber-50 dark:bg-amber-900/30 text-amber-700 dark:text-amber-400'
                : 'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
            )}
          >
            JSON
          </button>
        </div>
      )}
    </div>
  )
}
