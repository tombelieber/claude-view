import type { SerializedDockview } from 'dockview-react'
import { useCallback, useState } from 'react'

const PRESETS_STORAGE_KEY = 'claude-view:monitor-presets'

export function useLayoutPresets() {
  const [customPresets, setCustomPresets] = useState<Record<string, SerializedDockview>>(() => {
    const stored = localStorage.getItem(PRESETS_STORAGE_KEY)
    if (stored) {
      try {
        return JSON.parse(stored)
      } catch {
        return {}
      }
    }
    return {}
  })

  const savePreset = useCallback((name: string, layout: SerializedDockview) => {
    setCustomPresets((prev) => {
      const next = { ...prev, [name]: layout }
      try {
        localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next))
      } catch {
        /* QuotaExceeded */
      }
      return next
    })
  }, [])

  const deletePreset = useCallback((name: string) => {
    setCustomPresets((prev) => {
      const next = { ...prev }
      delete next[name]
      try {
        localStorage.setItem(PRESETS_STORAGE_KEY, JSON.stringify(next))
      } catch {
        /* QuotaExceeded */
      }
      return next
    })
  }, [])

  return { customPresets, savePreset, deletePreset }
}
