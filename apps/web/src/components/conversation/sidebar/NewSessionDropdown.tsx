import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import { ChevronDown, Loader2, MessageSquarePlus, Terminal } from 'lucide-react'
import { useCallback, useState } from 'react'

export type SessionMode = 'sdk' | 'tmux'

const STORAGE_KEY = 'claude-view:new-session-mode'

function readLastMode(): SessionMode {
  try {
    const v = localStorage.getItem(STORAGE_KEY)
    if (v === 'sdk' || v === 'tmux') return v
  } catch {}
  return 'sdk'
}

function saveLastMode(mode: SessionMode) {
  try {
    localStorage.setItem(STORAGE_KEY, mode)
  } catch {}
}

interface NewSessionDropdownProps {
  onNewChat: () => void
  onNewCliSession: () => Promise<void>
}

export function NewSessionDropdown({ onNewChat, onNewCliSession }: NewSessionDropdownProps) {
  const [mode, setMode] = useState<SessionMode>(readLastMode)
  const [isCreating, setIsCreating] = useState(false)

  const handlePrimaryClick = useCallback(async () => {
    if (isCreating) return
    if (mode === 'tmux') {
      setIsCreating(true)
      try {
        await onNewCliSession()
      } finally {
        setIsCreating(false)
      }
    } else {
      onNewChat()
    }
  }, [mode, onNewChat, onNewCliSession, isCreating])

  const handleSelect = useCallback(
    async (selected: SessionMode) => {
      setMode(selected)
      saveLastMode(selected)
      if (selected === 'tmux') {
        setIsCreating(true)
        try {
          await onNewCliSession()
        } finally {
          setIsCreating(false)
        }
      } else {
        onNewChat()
      }
    },
    [onNewChat, onNewCliSession],
  )

  const Icon = isCreating ? Loader2 : mode === 'tmux' ? Terminal : MessageSquarePlus
  const label = mode === 'tmux' ? 'New CLI Session' : 'New Chat'

  return (
    <div className="flex items-center rounded-md hover:bg-gray-200 dark:hover:bg-gray-800 transition-colors">
      {/* Primary action — full label */}
      <button
        type="button"
        onClick={handlePrimaryClick}
        disabled={isCreating}
        className="flex items-center gap-1.5 pl-2 pr-1 py-1.5 text-xs text-gray-600 dark:text-gray-300 disabled:text-gray-400 dark:disabled:text-gray-500"
        title={label}
      >
        <Icon size={13} className={`shrink-0 ${isCreating ? 'animate-spin' : ''}`} />
        <span className="whitespace-nowrap">{isCreating ? 'Starting...' : label}</span>
      </button>

      {/* Dropdown chevron */}
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            disabled={isCreating}
            className="flex items-center px-1 py-1.5 text-gray-400 dark:text-gray-500 border-l border-gray-300 dark:border-gray-700 disabled:opacity-50"
            title="Choose session type"
          >
            <ChevronDown size={12} />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg p-1 text-xs z-50"
            sideOffset={4}
            align="end"
          >
            <DropdownMenu.Item
              className="flex items-center gap-2 px-3 py-1.5 rounded cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 outline-none"
              onSelect={() => handleSelect('sdk')}
            >
              <MessageSquarePlus size={13} />
              <span>New Chat (Agent SDK)</span>
              {mode === 'sdk' && <span className="ml-auto text-blue-500">*</span>}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="flex items-center gap-2 px-3 py-1.5 rounded cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 outline-none"
              onSelect={() => handleSelect('tmux')}
            >
              <Terminal size={13} />
              <span>New CLI Session (tmux)</span>
              {mode === 'tmux' && <span className="ml-auto text-blue-500">*</span>}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  )
}
