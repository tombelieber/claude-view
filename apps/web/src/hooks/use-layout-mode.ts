import type { SerializedDockview } from 'dockview-react'
import { useCallback, useState } from 'react'

export type LayoutMode = 'auto-grid' | 'custom'

interface UseLayoutModeResult {
  mode: LayoutMode
  setMode: (mode: LayoutMode) => void
  toggleMode: () => void

  /** dockview serialized layout. null when mode is 'auto-grid' or no layout saved. */
  savedLayout: SerializedDockview | null
  setSavedLayout: (layout: SerializedDockview | null) => void

  /** Active preset name, or null if layout has been manually modified. */
  activePreset: string | null
  setActivePreset: (name: string | null) => void
}

const LAYOUT_STORAGE_KEY = 'claude-view:monitor-layout'
const MODE_STORAGE_KEY = 'claude-view:monitor-layout-mode'

export function useLayoutMode(): UseLayoutModeResult {
  const [mode, setModeState] = useState<LayoutMode>(() => {
    const stored = localStorage.getItem(MODE_STORAGE_KEY)
    return stored === 'custom' ? 'custom' : 'auto-grid'
  })

  const [savedLayout, setSavedLayoutState] = useState<SerializedDockview | null>(() => {
    const stored = localStorage.getItem(LAYOUT_STORAGE_KEY)
    if (stored) {
      try {
        return JSON.parse(stored)
      } catch {
        return null
      }
    }
    return null
  })

  const [activePreset, setActivePreset] = useState<string | null>(null)

  const setMode = useCallback((newMode: LayoutMode) => {
    setModeState(newMode)
    try {
      localStorage.setItem(MODE_STORAGE_KEY, newMode)
    } catch {
      /* QuotaExceeded — state still works in-memory */
    }
  }, [])

  const toggleMode = useCallback(() => {
    setMode(mode === 'auto-grid' ? 'custom' : 'auto-grid')
  }, [mode, setMode])

  const setSavedLayout = useCallback((layout: SerializedDockview | null) => {
    setSavedLayoutState(layout)
    setActivePreset(null) // manual change invalidates preset
    try {
      if (layout) {
        localStorage.setItem(LAYOUT_STORAGE_KEY, JSON.stringify(layout))
      } else {
        localStorage.removeItem(LAYOUT_STORAGE_KEY)
      }
    } catch {
      /* QuotaExceeded — layout persisted in-memory for this session */
    }
  }, [])

  return { mode, setMode, toggleMode, savedLayout, setSavedLayout, activePreset, setActivePreset }
}
