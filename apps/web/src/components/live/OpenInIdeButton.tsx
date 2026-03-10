import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, ExternalLink } from 'lucide-react'
import { useState } from 'react'
import { useIdePreference } from '../../hooks/use-ide-preference'
import { cn } from '../../lib/utils'

interface OpenInIdeButtonProps {
  projectPath: string
  filePath?: string
  compact?: boolean
}

export function OpenInIdeButton({ projectPath, filePath, compact }: OpenInIdeButtonProps) {
  const { availableIdes, preferredIde, setPreferredIde, openWithIde } = useIdePreference()
  const [open, setOpen] = useState(false)

  if (!preferredIde || availableIdes.length === 0) return null

  const handleOpen = async (ideId?: string) => {
    const targetId = ideId ?? preferredIde.id
    if (ideId) setPreferredIde(ideId)
    try {
      await openWithIde(targetId, projectPath, filePath)
    } catch (e) {
      console.warn('Failed to open IDE:', e)
    }
    setOpen(false)
  }

  const label = compact ? preferredIde.name : `Open in ${preferredIde.name}`

  return (
    <div className="inline-flex items-center" onClick={(e) => e.stopPropagation()}>
      {/* Main button */}
      <button
        type="button"
        onClick={() => handleOpen()}
        title={label}
        className={cn(
          'inline-flex items-center gap-1 transition-colors cursor-pointer',
          'text-gray-500 dark:text-gray-400 hover:text-indigo-600 dark:hover:text-indigo-400',
          compact ? 'p-0.5 rounded' : 'px-1.5 py-0.5 rounded text-[10px] font-medium',
        )}
      >
        <ExternalLink className={compact ? 'w-3 h-3' : 'w-3.5 h-3.5'} />
        {!compact && <span>{preferredIde.name}</span>}
      </button>

      {/* Dropdown chevron — only if multiple IDEs */}
      {availableIdes.length > 1 && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger asChild>
            <button
              type="button"
              className={cn(
                'p-0.5 rounded transition-colors cursor-pointer',
                'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
              )}
            >
              <ChevronDown className="w-3 h-3" />
            </button>
          </Popover.Trigger>
          <Popover.Portal>
            <Popover.Content
              align="end"
              sideOffset={4}
              className="z-50 w-40 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg py-1"
            >
              {availableIdes.map((ide) => (
                <button
                  key={ide.id}
                  type="button"
                  onClick={() => handleOpen(ide.id)}
                  className={cn(
                    'w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors cursor-pointer',
                    ide.id === preferredIde.id
                      ? 'text-indigo-600 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/20'
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                  )}
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  {ide.name}
                </button>
              ))}
            </Popover.Content>
          </Popover.Portal>
        </Popover.Root>
      )}
    </div>
  )
}
