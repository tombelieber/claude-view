import { useCallback, useRef } from 'react'
import type { DockviewApi, SerializedDockview } from 'dockview-react'

/**
 * Shared dockview persistence — attaches debounced listeners that serialize
 * layout state on every structural change (add/remove panel, resize, tab switch).
 *
 * Usage:
 *   const attachListeners = useDockviewPersistence(saveChatLayout)
 *   // inside onReady handler:
 *   attachListeners(event.api)
 */
export function useDockviewPersistence(
  onSave: (layout: SerializedDockview) => void,
  debounceMs = 100,
) {
  const onSaveRef = useRef(onSave)
  onSaveRef.current = onSave

  return useCallback(
    (api: DockviewApi) => {
      let timer: ReturnType<typeof setTimeout> | null = null
      const persist = () => {
        if (timer) clearTimeout(timer)
        timer = setTimeout(() => onSaveRef.current(api.toJSON()), debounceMs)
      }
      api.onDidAddPanel(persist)
      api.onDidRemovePanel(persist)
      api.onDidLayoutChange(persist)
      api.onDidActivePanelChange(persist)
    },
    [debounceMs],
  )
}
