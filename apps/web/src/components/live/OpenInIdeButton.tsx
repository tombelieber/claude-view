import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, ExternalLink, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'
import { useIdePreference } from '../../hooks/use-ide-preference'
import { TOAST_DURATION } from '../../lib/notify'
import { cn } from '../../lib/utils'

interface OpenInIdeButtonProps {
  projectPath: string
  filePath?: string
  compact?: boolean
}

export function OpenInIdeButton({ projectPath, filePath, compact }: OpenInIdeButtonProps) {
  const { availableIdes, preferredIde, setPreferredIde, openWithIde } = useIdePreference()
  const [open, setOpen] = useState(false)
  const [isOpening, setIsOpening] = useState(false)

  if (!preferredIde || availableIdes.length === 0) return null

  const handleOpen = async (ideId?: string) => {
    if (isOpening) return
    const targetId = ideId ?? preferredIde.id
    const targetName = availableIdes.find((i) => i.id === targetId)?.name ?? targetId
    if (ideId) setPreferredIde(ideId)
    setIsOpening(true)
    try {
      await openWithIde(targetId, projectPath, filePath)
      toast.success(`Opening ${targetName}`, { duration: TOAST_DURATION.micro })
    } catch (e) {
      toast.error(`Failed to open ${targetName}`, {
        description: e instanceof Error ? e.message : 'Unknown error',
        duration: TOAST_DURATION.extended,
      })
    } finally {
      setIsOpening(false)
      setOpen(false)
    }
  }

  const label = compact ? preferredIde.name : `Open in ${preferredIde.name}`
  const Icon = isOpening ? Loader2 : ExternalLink
  const iconClass = isOpening ? 'animate-spin' : ''

  return (
    <div className="inline-flex items-center" onClick={(e) => e.stopPropagation()}>
      {/* Main button */}
      <button
        type="button"
        onClick={() => handleOpen()}
        disabled={isOpening}
        title={label}
        className={cn(
          'inline-flex items-center gap-1 transition-colors cursor-pointer',
          'text-gray-500 dark:text-gray-400 hover:text-indigo-600 dark:hover:text-indigo-400',
          compact ? 'p-0.5 rounded' : 'px-1.5 py-0.5 rounded text-[10px] font-medium',
          isOpening && 'opacity-50 cursor-wait',
        )}
      >
        <Icon className={cn(compact ? 'w-3 h-3' : 'w-3.5 h-3.5', iconClass)} />
        {!compact && <span>{preferredIde.name}</span>}
      </button>

      {/* Dropdown chevron — only if multiple IDEs */}
      {availableIdes.length > 1 && (
        <Popover.Root open={open} onOpenChange={setOpen}>
          <Popover.Trigger asChild>
            <button
              type="button"
              disabled={isOpening}
              className={cn(
                'p-0.5 rounded transition-colors cursor-pointer',
                'text-gray-400 dark:text-gray-500 hover:text-gray-600 dark:hover:text-gray-300',
                isOpening && 'opacity-50 cursor-wait',
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
                  disabled={isOpening}
                  onClick={() => handleOpen(ide.id)}
                  className={cn(
                    'w-full flex items-center gap-2 px-3 py-1.5 text-xs transition-colors cursor-pointer',
                    ide.id === preferredIde.id
                      ? 'text-indigo-600 dark:text-indigo-400 bg-indigo-50 dark:bg-indigo-900/20'
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800',
                    isOpening && 'opacity-50 cursor-wait',
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
