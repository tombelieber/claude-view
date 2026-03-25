import * as DropdownMenu from '@radix-ui/react-dropdown-menu'
import type { IDockviewHeaderActionsProps } from 'dockview-core'
import { LayoutGrid } from 'lucide-react'

export function TabBarActions({ containerApi, activePanel }: IDockviewHeaderActionsProps) {
  const splitActive = (direction: 'right' | 'below') => {
    if (!activePanel) return
    containerApi.addPanel({
      id: `chat-new-${Date.now()}`,
      component: 'chat',
      title: 'New Chat',
      params: { sessionId: '' },
      position: { referencePanel: activePanel, direction },
    })
  }

  return (
    <div className="flex items-center gap-1 px-2">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button type="button" className="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700">
            <LayoutGrid className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500" />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg p-1 text-xs">
            <DropdownMenu.Item
              className="px-3 py-1.5 rounded cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 outline-none"
              onSelect={() => splitActive('right')}
            >
              Split Right
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className="px-3 py-1.5 rounded cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700 outline-none"
              onSelect={() => splitActive('below')}
            >
              Split Down
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  )
}
