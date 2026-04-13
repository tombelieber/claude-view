import {
  type DockviewApi,
  DockviewReact,
  type DockviewReadyEvent,
  type IWatermarkPanelProps,
  type SerializedDockview,
} from 'dockview-react'
import { useCallback, useRef } from 'react'
import { useDockviewPersistence } from '../../hooks/use-dockview-persistence'
import { ChatPanel } from './ChatPanel'
import { ChatTabRenderer } from './ChatTabRenderer'
import { TabBarActions } from './TabBarActions'

// Component registries — defined outside the component to avoid
// re-creating on every render (dockview uses referential equality).
const chatComponents = { chat: ChatPanel }
const chatTabComponents = { chat: ChatTabRenderer }

// Custom dockview theme — must be passed via `theme` prop to prevent dockview
// from defaulting to themeAbyss (which adds .dockview-theme-abyss and overrides
// our light-mode CSS variables in dockview-theme.css).
const cvTheme = { name: 'cv', className: 'dockview-theme-cv' }

interface ChatDockLayoutProps {
  /** Serialized layout to restore on mount. Null = empty dock. */
  initialLayout: SerializedDockview | null
  /** Called with the DockviewApi once dockview is ready. */
  onReady?: (api: DockviewApi) => void
  /** Persist layout on every structural change. */
  onLayoutChange: (layout: SerializedDockview) => void
}

function ChatWatermark(_props: IWatermarkPanelProps) {
  return (
    <div className="flex items-center justify-center h-full">
      <div className="w-full max-w-2xl px-4">
        <p className="text-center text-sm text-gray-400 dark:text-gray-500 mb-4">
          Select a session from the sidebar, or start a new conversation.
        </p>
      </div>
    </div>
  )
}

export function ChatDockLayout({ initialLayout, onReady, onLayoutChange }: ChatDockLayoutProps) {
  const onReadyRef = useRef(onReady)
  onReadyRef.current = onReady

  const attachListeners = useDockviewPersistence(onLayoutChange)

  // onReady fires ONCE per dockview instance. All mutable values are read
  // via refs so the callback identity is stable and dockview never re-initializes.
  // Persistence listeners are attached here (not in a React effect) so they
  // are immune to React StrictMode double-invoke — each dockview instance gets
  // its own listeners that persist for its lifetime.
  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      if (initialLayout) {
        try {
          event.api.fromJSON(initialLayout)
        } catch {
          // Corrupt or incompatible layout — start fresh
          event.api.clear()
        }
      }

      onReadyRef.current?.(event.api)
      attachListeners(event.api)
    },
    // initialLayout is read once at mount — changes after mount are irrelevant
    // (the layout is already live). onReadyRef is stable.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [initialLayout, attachListeners],
  )

  return (
    <DockviewReact
      className="flex-1 min-w-0"
      theme={cvTheme}
      components={chatComponents}
      tabComponents={chatTabComponents}
      defaultTabComponent={ChatTabRenderer}
      onReady={handleReady}
      watermarkComponent={ChatWatermark}
      rightHeaderActionsComponent={TabBarActions}
    />
  )
}
