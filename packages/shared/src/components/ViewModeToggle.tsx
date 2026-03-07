import { cn } from '../utils/cn'

interface ViewModeToggleProps {
  verboseMode: boolean
  onToggleVerbose: () => void
  richRenderMode?: 'rich' | 'json'
  onSetRichRenderMode?: (mode: 'rich' | 'json') => void
  className?: string
}

/**
 * Chat/Debug + Rich/JSON segmented controls.
 * Prop-driven — no store dependency. The consuming app wires state.
 *
 * - Chat = filtered conversation (user + assistant only)
 * - Debug = full execution trace (all message roles)
 * - Rich = formatted renderers (Debug only)
 * - JSON = raw JSON view (Debug only)
 */
export function ViewModeToggle({
  verboseMode,
  onToggleVerbose,
  richRenderMode,
  onSetRichRenderMode,
  className,
}: ViewModeToggleProps) {
  return (
    <div className={cn('flex items-center gap-1', className)}>
      {/* Chat / Debug segmented control */}
      <div className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
        <button
          type="button"
          onClick={() => verboseMode && onToggleVerbose()}
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
          type="button"
          onClick={() => !verboseMode && onToggleVerbose()}
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
      {verboseMode && onSetRichRenderMode && (
        <div className="flex items-center rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden">
          <button
            type="button"
            onClick={() => richRenderMode !== 'rich' && onSetRichRenderMode('rich')}
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
            type="button"
            onClick={() => richRenderMode !== 'json' && onSetRichRenderMode('json')}
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
