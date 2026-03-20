import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IWatermarkPanelProps,
} from 'dockview-react'
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

function ChatWatermark(_props: IWatermarkPanelProps) {
  return (
    <div className="flex items-center justify-center h-full">
      <div className="w-full max-w-2xl px-4">
        <p className="text-center text-sm text-gray-400 dark:text-[#8B949E] mb-4">
          Select a session from the sidebar, or start a new conversation.
        </p>
      </div>
    </div>
  )
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
      watermarkComponent={ChatWatermark}
      rightHeaderActionsComponent={TabBarActions}
    />
  )
}
