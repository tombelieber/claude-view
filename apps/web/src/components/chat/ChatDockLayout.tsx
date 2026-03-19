import { type DockviewApi, DockviewReact, type DockviewReadyEvent } from 'dockview-react'
import { useCallback, useRef } from 'react'
import { ChatPanel } from './ChatPanel'
import { ChatTabRenderer } from './ChatTabRenderer'
import { TabBarActions } from './TabBarActions'

// Component registries — defined outside the component to avoid
// re-creating on every render (dockview uses referential equality).
const chatComponents = { chat: ChatPanel }
const chatTabComponents = { chat: ChatTabRenderer }

interface ChatDockLayoutProps {
  onReady?: (api: DockviewApi) => void
}

export function ChatDockLayout({ onReady }: ChatDockLayoutProps) {
  const apiRef = useRef<DockviewApi | null>(null)

  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api
      onReady?.(event.api)
    },
    [onReady],
  )

  return (
    <DockviewReact
      className="dockview-theme-cv flex-1"
      components={chatComponents}
      tabComponents={chatTabComponents}
      defaultTabComponent={ChatTabRenderer}
      onReady={handleReady}
      rightHeaderActionsComponent={TabBarActions}
    />
  )
}
