import * as Popover from '@radix-ui/react-popover'
import { Check, ChevronDown, Cpu } from 'lucide-react'
import { useState } from 'react'
import { type ModelOption, useModelOptions } from '../../hooks/use-models'
import { cn } from '../../lib/utils'

export type { ModelOption }

interface ModelSelectorProps {
  model: string
  onModelChange: (model: string) => void
  models?: ModelOption[]
  disabled?: boolean
}

function getLabel(models: ModelOption[], modelId: string): string {
  const found = models.find((m) => m.id === modelId)
  return found?.label ?? modelId
}

/**
 * Model selector chip with popover dropdown.
 * Shows model name, description, and context window size.
 */
export function ModelSelector({ model, onModelChange, models, disabled }: ModelSelectorProps) {
  const [open, setOpen] = useState(false)
  const { options: fetchedModels, isLoading } = useModelOptions()
  const options = models ?? fetchedModels
  const label = getLabel(options, model)

  if (isLoading && options.length === 0) {
    return (
      <div className="flex items-center gap-1.5 text-xs text-text-secondary">
        <div className="h-3.5 w-24 animate-pulse rounded bg-bg-tertiary" />
      </div>
    )
  }

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
          <span title={model}>{label}</span>
          <ChevronDown className="w-3 h-3" aria-hidden="true" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          side="top"
          sideOffset={6}
          align="start"
          className="z-50 w-72 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1.5 animate-in fade-in-0 zoom-in-95"
        >
          <div className="px-2.5 py-1.5 text-xs font-medium text-gray-400 dark:text-gray-500">
            Select a model
          </div>
          {options.map((opt) => {
            const isActive = opt.id === model
            return (
              <Popover.Close key={opt.id} asChild>
                <button
                  type="button"
                  onClick={() => onModelChange(opt.id)}
                  className={cn(
                    'flex items-start justify-between w-full px-2.5 py-2 rounded-md transition-colors cursor-pointer',
                    isActive
                      ? 'bg-gray-50 dark:bg-gray-800/60'
                      : 'hover:bg-gray-50 dark:hover:bg-gray-800/40',
                  )}
                >
                  <div className="flex flex-col gap-0.5 text-left min-w-0">
                    <span
                      className={cn(
                        'text-sm leading-tight',
                        isActive
                          ? 'font-semibold text-gray-900 dark:text-gray-100'
                          : 'font-medium text-gray-800 dark:text-gray-200',
                      )}
                    >
                      {opt.label}
                    </span>
                    {(opt.description || opt.contextWindow) && (
                      <span className="text-xs text-gray-400 dark:text-gray-500 leading-tight">
                        {opt.description}
                        {opt.description && opt.contextWindow && ' · '}
                        {opt.contextWindow && `${opt.contextWindow} context`}
                      </span>
                    )}
                  </div>
                  {isActive && (
                    <Check className="w-4 h-4 text-gray-500 dark:text-gray-400 shrink-0 mt-0.5" />
                  )}
                </button>
              </Popover.Close>
            )
          })}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
