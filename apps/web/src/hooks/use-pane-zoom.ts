import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useState } from 'react'

interface UsePaneZoomOptions {
  api: DockviewApi | null
}

export function usePaneZoom({ api }: UsePaneZoomOptions) {
  const [isZoomed, setIsZoomed] = useState(() => api?.hasMaximizedGroup() ?? false)

  useEffect(() => {
    if (!api) {
      setIsZoomed(false)
      return
    }
    setIsZoomed(api.hasMaximizedGroup())
    const disposable = api.onDidMaximizedGroupChange((e) => {
      setIsZoomed(e.isMaximized)
    })
    return () => disposable.dispose()
  }, [api])

  const toggleZoom = useCallback(() => {
    if (!api) return
    if (api.hasMaximizedGroup()) {
      api.exitMaximizedGroup()
    } else {
      api.activePanel?.api.maximize()
    }
  }, [api])

  const zoomPanel = useCallback(
    (panelId: string) => {
      if (!api) return
      const panel = api.getPanel(panelId)
      panel?.api.maximize()
    },
    [api],
  )

  const exitZoom = useCallback(() => {
    if (!api) return
    api.exitMaximizedGroup()
  }, [api])

  return { isZoomed, toggleZoom, zoomPanel, exitZoom }
}
