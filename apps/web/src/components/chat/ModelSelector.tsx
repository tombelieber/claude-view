import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, Cpu } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'

export interface ModelOption {
  id: string
  label: string
}

interface ModelSelectorProps {
  model: string
  onModelChange: (model: string) => void
  models?: ModelOption[]
  disabled?: boolean
}

const DEFAULT_MODELS: ModelOption[] = [
  { id: 'claude-opus-4-6', label: 'Opus 4.6' },
  { id: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
  { id: 'claude-haiku-4-5', label: 'Haiku 4.5' },
]

function getLabel(models: ModelOption[], modelId: string): string {
  const found = models.find((m) => m.id === modelId)
  return found?.label ?? modelId
}

/**
 * Model selector chip with popover dropdown.
 * Displays as: [Opus 4.6 ▾]
 */
export function ModelSelector({ model, onModelChange, models, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false)
  const options = models ?? DEFAULT_MODELS
  const label = getLabel(options, model)

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            'inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium transition-colors duration-150',
            'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
            'border border-gray-200 dark:border-gray-700',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            disabled
              ? 'opacity-50 cursor-not-allowed'
              : 'cursor-pointer hover:border-gray-300 dark:hover:border-gray-600',
          )}
          aria-label={`Model: ${label}. Click to change.`}
        >
          <Cpu className="w-3 h-3" aria-hidden="true" />
          <span>{label}</span>
          <ChevronDown className="w-3 h-3" aria-hidden="true" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          side="top"
          sideOffset={6}
          align="start"
          className="z-50 w-48 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
        >
          {options.map((opt) => {
            const isActive = opt.id === model
            return (
              <Popover.Close key={opt.id} asChild>
                <button
                  type="button"
                  onClick={() => onModelChange(opt.id)}
                  className={cn(
                    'flex items-center gap-2 w-full px-3 py-2 text-sm rounded-md transition-colors cursor-pointer',
                    isActive
                      ? 'bg-blue-50 dark:bg-blue-950/30 text-blue-700 dark:text-blue-300 font-medium'
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                >
                  <Cpu className="w-4 h-4" aria-hidden="true" />
                  <span>{opt.label}</span>
                </button>
              </Popover.Close>
            )
          })}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
