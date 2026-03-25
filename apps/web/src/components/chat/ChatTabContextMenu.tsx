import * as ContextMenu from '@radix-ui/react-context-menu'
import type { DockviewApi, IDockviewPanel } from 'dockview-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'

export interface ChatTabContextMenuProps {
  children: ReactNode
  panel: IDockviewPanel
  api: DockviewApi
}

export function ChatTabContextMenu({ children, panel, api }: ChatTabContextMenuProps) {
  const sessionId = (panel.params as { sessionId?: string })?.sessionId

  const handleClose = () => {
    panel.api.close()
  }

  const handleCloseOthers = () => {
    const toClose = api.panels.filter((p) => p.id !== panel.id)
    for (const p of toClose) {
      p.api.close()
    }
  }

  const handleCloseAll = () => {
    const allPanels = [...api.panels]
    for (const p of allPanels) {
      p.api.close()
    }
  }

  const handleSplitRight = () => {
    if (!sessionId) return
    api.addPanel({
      id: `chat-${sessionId}-split-r-${Date.now()}`,
      component: 'chat',
      title: panel.title ?? sessionId.slice(0, 8),
      params: {
        sessionId,
        liveStatus: (panel.params as { liveStatus?: string })?.liveStatus ?? 'inactive',
      },
      position: { referencePanel: panel.id, direction: 'right' },
    })
  }

  const handleSplitDown = () => {
    if (!sessionId) return
    api.addPanel({
      id: `chat-${sessionId}-split-d-${Date.now()}`,
      component: 'chat',
      title: panel.title ?? sessionId.slice(0, 8),
      params: {
        sessionId,
        liveStatus: (panel.params as { liveStatus?: string })?.liveStatus ?? 'inactive',
      },
      position: { referencePanel: panel.id, direction: 'below' },
    })
  }

  const handleCopySessionId = () => {
    if (sessionId) {
      navigator.clipboard.writeText(sessionId).then(
        () => toast.success('Session ID copied'),
        () => toast.error('Failed to copy'),
      )
    }
  }

  const handleEndSession = () => {
    if (sessionId) {
      fetch(`/api/sidecar/sessions/${sessionId}`, { method: 'DELETE' }).catch((err) => {
        toast.error('Failed to end session', { description: String(err) })
      })
      panel.api.close()
    }
  }

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="min-w-[180px] rounded-md bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 shadow-lg py-1 text-xs z-[100]">
          <MenuItem label="Close" shortcut="Ctrl+W" onClick={handleClose} />
          <MenuItem label="Close Others" onClick={handleCloseOthers} />
          <MenuItem label="Close All" onClick={handleCloseAll} />

          <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

          <MenuItem label="Split Right" shortcut="Ctrl+\" onClick={handleSplitRight} />
          <MenuItem label="Split Down" shortcut="Ctrl+Shift+\" onClick={handleSplitDown} />

          <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

          <MenuItem label="Copy Session ID" onClick={handleCopySessionId} />
          <MenuItem label="End Session" onClick={handleEndSession} destructive />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  )
}

function MenuItem({
  label,
  shortcut,
  onClick,
  destructive,
}: {
  label: string
  shortcut?: string
  onClick: () => void
  destructive?: boolean
}) {
  return (
    <ContextMenu.Item
      onSelect={onClick}
      className={`flex items-center justify-between px-3 py-1.5 cursor-pointer outline-none ${
        destructive
          ? 'text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20'
          : 'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800'
      }`}
    >
      <span>{label}</span>
      {shortcut && (
        <span className="ml-4 text-xs text-gray-400 dark:text-gray-500">{shortcut}</span>
      )}
    </ContextMenu.Item>
  )
}
