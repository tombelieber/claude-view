import { Code, MessageSquare } from 'lucide-react'
import { cn } from '../../lib/utils'
import { useMonitorStore } from '../../store/monitor-store'

interface DisplayModeToggleProps {
  className?: string
}

export function DisplayModeToggle({ className }: DisplayModeToggleProps) {
  const displayMode = useMonitorStore((s) => s.displayMode)
  const setDisplayMode = useMonitorStore((s) => s.setDisplayMode)

  return (
    <div
      className={cn(
        'inline-flex rounded-md border border-gray-200 dark:border-gray-700',
        className,
      )}
    >
      <button
        type="button"
        onClick={() => setDisplayMode('chat')}
        className={cn(
          'px-2 py-0.5 text-xs font-medium rounded-l-md transition-colors',
          displayMode === 'chat'
            ? 'bg-blue-500/10 text-blue-400 border-r border-blue-500/30'
            : 'text-gray-400 hover:text-gray-300 border-r border-gray-200 dark:border-gray-700',
        )}
      >
        <MessageSquare className="w-3 h-3 inline mr-0.5 -mt-0.5" />
        Chat
      </button>
      <button
        type="button"
        onClick={() => setDisplayMode('developer')}
        className={cn(
          'px-2 py-0.5 text-xs font-medium rounded-r-md transition-colors',
          displayMode === 'developer'
            ? 'bg-purple-500/10 text-purple-400'
            : 'text-gray-400 hover:text-gray-300',
        )}
      >
        <Code className="w-3 h-3 inline mr-0.5 -mt-0.5" />
        Developer
      </button>
    </div>
  )
}
