import * as ContextMenu from '@radix-ui/react-context-menu'
import type { DockviewApi, IDockviewPanel } from 'dockview-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'
import { useSessionMutations } from '../../hooks/use-session-mutations'

export interface TabContextMenuProps {
  children: ReactNode
  panel: IDockviewPanel
  api: DockviewApi
  /** Component type for split panels (e.g. 'chat', 'session'). */
  splitComponent: string
}

export function TabContextMenu({ children, panel, api, splitComponent }: TabContextMenuProps) {
  const params = panel.params as Record<string, unknown>
  const sessionId = (params.sessionId as string | undefined) ?? undefined
  const { deleteSession } = useSessionMutations()

  const isMaximized = panel.api.isMaximized()

  const handleZoom = () => {
    if (isMaximized) {
      panel.api.exitMaximized()
    } else {
      panel.api.maximize()
    }
  }

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

  const handleSplit = (direction: 'right' | 'below') => {
    if (!sessionId) return
    const dir = direction === 'right' ? 'right' : 'below'
    api.addPanel({
      id: `${splitComponent}-${sessionId}-split-${dir[0]}-${Date.now()}`,
      component: splitComponent,
      title: panel.title ?? sessionId.slice(0, 8),
      params: { ...params },
      position: { referencePanel: panel.id, direction: dir },
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
      deleteSession.mutate(sessionId)
      panel.api.close()
    }
  }

  // Session metadata from panel params
  const projectName = params.projectName as string | undefined
  const branch = params.branch as string | undefined

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="min-w-[180px] rounded-md bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 shadow-lg py-1 text-xs z-[100]">
          <MenuItem
            label={isMaximized ? 'Exit Zoom' : 'Zoom Pane'}
            shortcut="Ctrl+Shift+Enter"
            onClick={handleZoom}
          />

          <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

          <MenuItem label="Split Right" shortcut="Ctrl+D" onClick={() => handleSplit('right')} />
          <MenuItem
            label="Split Down"
            shortcut="Ctrl+Shift+D"
            onClick={() => handleSplit('below')}
          />

          <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

          {(projectName || branch) && (
            <>
              <div className="px-3 py-1 text-gray-400 dark:text-gray-500 space-y-0.5">
                {projectName && <div>{projectName}</div>}
                {branch && <div>Branch: {branch}</div>}
              </div>
              <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />
            </>
          )}

          <MenuItem label="Copy Session ID" onClick={handleCopySessionId} />
          <MenuItem label="End Session" onClick={handleEndSession} destructive />

          <ContextMenu.Separator className="h-px bg-gray-200 dark:bg-gray-700 my-1" />

          <MenuItem label="Close" shortcut="Ctrl+Shift+W" onClick={handleClose} />
          <MenuItem label="Close Others" onClick={handleCloseOthers} />
          <MenuItem label="Close All" onClick={handleCloseAll} />
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
