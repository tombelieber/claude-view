import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, Code2, Lightbulb, MessageSquare } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'

type Mode = 'plan' | 'code' | 'ask'

interface ModeSwitchProps {
  mode: Mode
  onModeChange: (mode: Mode) => void
  disabled?: boolean
}

const MODE_CONFIG: Record<
  Mode,
  {
    label: string
    icon: typeof Lightbulb
    pillBg: string
    pillText: string
    activeBg: string
    activeText: string
  }
> = {
  plan: {
    label: 'Plan',
    icon: Lightbulb,
    pillBg: 'bg-amber-100 dark:bg-amber-950/40',
    pillText: 'text-amber-700 dark:text-amber-400',
    activeBg: 'bg-amber-50 dark:bg-amber-950/30',
    activeText: 'text-amber-700 dark:text-amber-300',
  },
  code: {
    label: 'Code',
    icon: Code2,
    pillBg: 'bg-emerald-100 dark:bg-emerald-950/40',
    pillText: 'text-emerald-700 dark:text-emerald-400',
    activeBg: 'bg-emerald-50 dark:bg-emerald-950/30',
    activeText: 'text-emerald-700 dark:text-emerald-300',
  },
  ask: {
    label: 'Ask',
    icon: MessageSquare,
    pillBg: 'bg-blue-100 dark:bg-blue-950/40',
    pillText: 'text-blue-700 dark:text-blue-400',
    activeBg: 'bg-blue-50 dark:bg-blue-950/30',
    activeText: 'text-blue-700 dark:text-blue-300',
  },
}

const MODES: Mode[] = ['plan', 'code', 'ask']

/**
 * Mode selector pill with popover dropdown.
 * Color-coded: plan=amber, code=green, ask=blue.
 */
export function ModeSwitch({ mode, onModeChange, disabled }: ModeSwitchProps) {
  const [open, setOpen] = useState(false)
  const config = MODE_CONFIG[mode]
  const Icon = config.icon

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          type="button"
          disabled={disabled}
          className={cn(
            'inline-flex items-center gap-1 px-2 py-1 rounded-full text-xs font-medium transition-colors duration-150',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
            config.pillBg,
            config.pillText,
            disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer hover:opacity-80',
          )}
          aria-label={`Mode: ${config.label}. Click to change.`}
        >
          <Icon className="w-3 h-3" aria-hidden="true" />
          <span>{config.label}</span>
          <ChevronDown className="w-3 h-3" aria-hidden="true" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          side="top"
          sideOffset={6}
          align="start"
          className="z-50 w-44 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
        >
          {MODES.map((m) => {
            const mc = MODE_CONFIG[m]
            const ModeIcon = mc.icon
            const isActive = m === mode
            return (
              <Popover.Close key={m} asChild>
                <button
                  type="button"
                  onClick={() => onModeChange(m)}
                  className={cn(
                    'flex items-center gap-2 w-full px-3 py-2 text-sm rounded-md transition-colors cursor-pointer',
                    isActive
                      ? `${mc.activeBg} ${mc.activeText} font-medium`
                      : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                  )}
                >
                  <ModeIcon className="w-4 h-4" aria-hidden="true" />
                  <span>{mc.label}</span>
                </button>
              </Popover.Close>
            )
          })}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
