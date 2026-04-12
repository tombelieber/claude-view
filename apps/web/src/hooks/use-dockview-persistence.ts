import { useCallback, useEffect, useRef } from 'react'
import type { DockviewApi, SerializedDockview } from 'dockview-react'

/**
 * Shared dockview persistence — attaches debounced listeners that serialize
 * layout state on every structural change (add/remove panel, resize, tab switch).
 *
 * Flushes pending saves on beforeunload, visibilitychange (hidden), and
 * component unmount so tab-change-then-refresh never loses state.
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
  const apiRef = useRef<DockviewApi | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const flush = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
    const api = apiRef.current
    if (!api) return
    try {
      onSaveRef.current(api.toJSON())
    } catch {
      // api disposed after unmount — nothing to save
    }
  }, [])

  useEffect(() => {
    const handleUnload = () => flush()
    const handleVisibility = () => {
      if (document.visibilityState === 'hidden') flush()
    }
    window.addEventListener('beforeunload', handleUnload)
    document.addEventListener('visibilitychange', handleVisibility)
    return () => {
      flush()
      window.removeEventListener('beforeunload', handleUnload)
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [flush])

  return useCallback(
    (api: DockviewApi) => {
      apiRef.current = api
      const persist = () => {
        if (timerRef.current) clearTimeout(timerRef.current)
        timerRef.current = setTimeout(() => {
          try {
            onSaveRef.current(api.toJSON())
          } catch {
            // api disposed — ignore
          }
          timerRef.current = null
        }, debounceMs)
      }
      api.onDidAddPanel(persist)
      api.onDidRemovePanel(persist)
      api.onDidLayoutChange(persist)
      api.onDidActivePanelChange(persist)
    },
    [debounceMs],
  )
}
