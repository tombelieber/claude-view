import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IWatermarkPanelProps,
  type SerializedDockview,
} from 'dockview-react'
import { useCallback, useRef } from 'react'
import { ChatPanel } from './ChatPanel'
import { ChatTabRenderer } from './ChatTabRenderer'
import { TabBarActions } from './TabBarActions'

// Component registries — defined outside the component to avoid
// re-creating on every render (dockview uses referential equality).
const chatComponents = { chat: ChatPanel }
const chatTabComponents = { chat: ChatTabRenderer }

const STORAGE_KEY = 'claude-view:chat-layout'

/** Read saved dockview layout from localStorage. */
export function readSavedChatLayout(): SerializedDockview | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return JSON.parse(raw) as SerializedDockview
  } catch {
    // Corrupt — start fresh
  }
  return null
}

/** Write dockview layout to localStorage. */
function saveChatLayout(layout: SerializedDockview): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout))
  } catch {
    // QuotaExceeded — best effort
  }
}

interface ChatDockLayoutProps {
  /** Serialized layout to restore on mount. Null = empty dock. */
  initialLayout: SerializedDockview | null
  /** Called with the DockviewApi once dockview is ready. */
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

export function ChatDockLayout({ initialLayout, onReady }: ChatDockLayoutProps) {
  const apiRef = useRef<DockviewApi | null>(null)
  const onReadyRef = useRef(onReady)
  onReadyRef.current = onReady

  // onReady fires ONCE per dockview instance. All mutable values are read
  // via refs so the callback identity is stable and dockview never re-initializes.
  // Persistence listeners are attached here (not in a React effect) so they
  // are immune to React StrictMode double-invoke — each dockview instance gets
  // its own listeners that persist for its lifetime.
  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      apiRef.current = event.api

      // Restore saved layout via dockview's native fromJSON
      if (initialLayout) {
        try {
          event.api.fromJSON(initialLayout)
        } catch {
          // Corrupt or incompatible layout — start fresh
          event.api.clear()
        }
      }

      // Notify parent (ChatPageV2) with the API handle
      onReadyRef.current?.(event.api)

      // Attach persistence listeners — debounced to batch rapid mutations
      let debounceTimer: ReturnType<typeof setTimeout> | null = null
      const persistLayout = () => {
        if (debounceTimer) clearTimeout(debounceTimer)
        debounceTimer = setTimeout(() => {
          if (apiRef.current) {
            saveChatLayout(apiRef.current.toJSON())
          }
        }, 100)
      }
      event.api.onDidAddPanel(persistLayout)
      event.api.onDidRemovePanel(persistLayout)
      event.api.onDidLayoutChange(persistLayout)
      event.api.onDidActivePanelChange(persistLayout)
    },
    // initialLayout is read once at mount — changes after mount are irrelevant
    // (the layout is already live). onReadyRef is stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [initialLayout],
  )

  return (
    <DockviewReact
      className="dockview-theme-cv flex-1 min-w-0"
      components={chatComponents}
      tabComponents={chatTabComponents}
      defaultTabComponent={ChatTabRenderer}
      onReady={handleReady}
      watermarkComponent={ChatWatermark}
      rightHeaderActionsComponent={TabBarActions}
    />
  )
}
