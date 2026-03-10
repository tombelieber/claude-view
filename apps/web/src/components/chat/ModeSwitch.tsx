import * as AlertDialog from '@radix-ui/react-alert-dialog'
import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, ClipboardList, FileEdit, Shield, ShieldOff, SkipForward } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'
import type { PermissionMode } from '../../types/control'

interface ModeSwitchProps {
  mode: PermissionMode
  onModeChange: (mode: PermissionMode) => void
  disabled?: boolean
}

const MODE_CONFIG: Record<
  PermissionMode,
  {
    label: string
    icon: typeof Shield
    description: string
    pillBg: string
    pillText: string
    activeBg: string
    activeText: string
  }
> = {
  default: {
    label: 'Default',
    icon: Shield,
    description: 'Prompts for dangerous operations',
    pillBg: 'bg-blue-100 dark:bg-blue-950/40',
    pillText: 'text-blue-700 dark:text-blue-400',
    activeBg: 'bg-blue-50 dark:bg-blue-950/30',
    activeText: 'text-blue-700 dark:text-blue-300',
  },
  acceptEdits: {
    label: 'Accept Edits',
    icon: FileEdit,
    description: 'Auto-approves file edits',
    pillBg: 'bg-teal-100 dark:bg-teal-950/40',
    pillText: 'text-teal-700 dark:text-teal-400',
    activeBg: 'bg-teal-50 dark:bg-teal-950/30',
    activeText: 'text-teal-700 dark:text-teal-300',
  },
  plan: {
    label: 'Plan',
    icon: ClipboardList,
    description: 'Plan only, no tool execution',
    pillBg: 'bg-amber-100 dark:bg-amber-950/40',
    pillText: 'text-amber-700 dark:text-amber-400',
    activeBg: 'bg-amber-50 dark:bg-amber-950/30',
    activeText: 'text-amber-700 dark:text-amber-300',
  },
  dontAsk: {
    label: 'Skip Dangerous',
    icon: SkipForward,
    description: 'Skips tools that need permission',
    pillBg: 'bg-gray-100 dark:bg-gray-800/60',
    pillText: 'text-gray-600 dark:text-gray-400',
    activeBg: 'bg-gray-50 dark:bg-gray-800/30',
    activeText: 'text-gray-600 dark:text-gray-300',
  },
  bypassPermissions: {
    label: 'Trust All',
    icon: ShieldOff,
    description: 'Auto-approves everything (dangerous)',
    pillBg: 'bg-red-100 dark:bg-red-950/40',
    pillText: 'text-red-700 dark:text-red-400',
    activeBg: 'bg-red-50 dark:bg-red-950/30',
    activeText: 'text-red-700 dark:text-red-300',
  },
}

const MODES: PermissionMode[] = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']

export function ModeSwitch({ mode, onModeChange, disabled }: ModeSwitchProps) {
  const [open, setOpen] = useState(false)
  const [confirmBypass, setConfirmBypass] = useState(false)
  const config = MODE_CONFIG[mode]
  const Icon = config.icon

  const handleSelect = (m: PermissionMode) => {
    if (m === 'bypassPermissions') {
      setConfirmBypass(true)
      return
    }
    onModeChange(m)
  }

  const handleConfirmBypass = () => {
    setConfirmBypass(false)
    onModeChange('bypassPermissions')
  }

  return (
    <>
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
            className="z-50 w-56 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
          >
            {MODES.map((m) => {
              const mc = MODE_CONFIG[m]
              const ModeIcon = mc.icon
              const isActive = m === mode
              return (
                <Popover.Close key={m} asChild>
                  <button
                    type="button"
                    onClick={() => handleSelect(m)}
                    className={cn(
                      'flex items-center gap-2 w-full px-3 py-2 text-sm rounded-md transition-colors cursor-pointer',
                      isActive
                        ? `${mc.activeBg} ${mc.activeText} font-medium`
                        : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
                    )}
                  >
                    <ModeIcon className="w-4 h-4 shrink-0" aria-hidden="true" />
                    <div className="text-left">
                      <div>{mc.label}</div>
                      <div className="text-[10px] opacity-70">{mc.description}</div>
                    </div>
                  </button>
                </Popover.Close>
              )
            })}
          </Popover.Content>
        </Popover.Portal>
      </Popover.Root>

      <AlertDialog.Root open={confirmBypass} onOpenChange={setConfirmBypass}>
        <AlertDialog.Portal>
          <AlertDialog.Overlay className="fixed inset-0 z-50 bg-black/40 animate-in fade-in-0" />
          <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-xl shadow-2xl p-6 max-w-sm mx-4 animate-in fade-in-0 zoom-in-95">
            <AlertDialog.Title className="flex items-center gap-2 mb-3">
              <ShieldOff className="w-5 h-5 text-red-500" />
              <span className="text-sm font-semibold text-gray-900 dark:text-gray-100">
                Enable Trust All Mode?
              </span>
            </AlertDialog.Title>
            <AlertDialog.Description className="text-xs text-gray-600 dark:text-gray-400 mb-4">
              This mode auto-approves ALL tool executions including destructive operations like file
              deletion and command execution. Use only when you fully trust the session.
            </AlertDialog.Description>
            <div className="flex gap-2 justify-end">
              <AlertDialog.Cancel asChild>
                <button
                  type="button"
                  className="px-3 py-1.5 text-xs font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 rounded-md hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
                >
                  Cancel
                </button>
              </AlertDialog.Cancel>
              <AlertDialog.Action asChild>
                <button
                  type="button"
                  onClick={handleConfirmBypass}
                  className="px-3 py-1.5 text-xs font-medium text-white bg-red-600 rounded-md hover:bg-red-700 transition-colors"
                >
                  Enable Trust All
                </button>
              </AlertDialog.Action>
            </div>
          </AlertDialog.Content>
        </AlertDialog.Portal>
      </AlertDialog.Root>
    </>
  )
}
