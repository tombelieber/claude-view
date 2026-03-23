import type { DockviewApi } from 'dockview-react'
import { useEffect, useRef } from 'react'
import { useMonitorStore } from '../../store/monitor-store'
import type { LiveSession } from './use-live-sessions'

interface UseMonitorKeyboardShortcutsOptions {
  enabled: boolean
  sessions: LiveSession[]
  onLayoutModeChange?: (mode: 'auto-grid' | 'custom') => void
  layoutMode?: 'auto-grid' | 'custom'
  dockviewApi?: DockviewApi | null
}

/** Max grid columns the user can set via keyboard. */
const MAX_COLS = 4
/** Min grid columns the user can set via keyboard. */
const MIN_COLS = 1

function isInputFocused(): boolean {
  const el = document.activeElement
  if (!el) return false
  const tag = el.tagName.toLowerCase()
  return (
    tag === 'input' || tag === 'textarea' || tag === 'select' || el.hasAttribute('contenteditable')
  )
}

/**
 * Monitor-specific keyboard shortcuts. Active only when the monitor view is showing.
 *
 * | Key     | Action                                       |
 * |---------|----------------------------------------------|
 * | 1-9     | Select pane by position                      |
 * | Enter   | Expand selected pane                         |
 * | Escape  | Close expanded pane, or deselect              |
 * | p       | Toggle pin on selected pane                  |
 * | h       | Hide selected pane                           |
 * | m       | Toggle verbose mode (global)                 |
 * | + / =   | Increase grid columns (up to 4)              |
 * | -       | Decrease grid columns (down to 1)            |
 */
export function useMonitorKeyboardShortcuts(options: UseMonitorKeyboardShortcutsOptions): void {
  const optionsRef = useRef(options)
  optionsRef.current = options

  useEffect(() => {
    function handler(e: KeyboardEvent) {
      const opts = optionsRef.current
      if (!opts.enabled) return

      const store = useMonitorStore.getState()

      // Escape always works, even in inputs
      if (e.key === 'Escape') {
        if (store.expandedPaneId) {
          store.expandPane(null)
          e.preventDefault()
        } else if (store.selectedPaneId) {
          store.selectPane(null)
          e.preventDefault()
        }
        return
      }

      // Skip all other shortcuts when an input is focused
      if (isInputFocused()) return

      // Ctrl+Shift combos for layout mode switching (must come BEFORE the modifier guard)
      if (e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey) {
        if (e.key === 'G' || e.key === 'g') {
          opts.onLayoutModeChange?.('auto-grid')
          e.preventDefault()
          return
        }
        if (e.key === 'L' || e.key === 'l') {
          opts.onLayoutModeChange?.('custom')
          e.preventDefault()
          return
        }
      }

      // Ignore key events with modifier keys (Ctrl, Alt, Meta)
      if (e.ctrlKey || e.altKey || e.metaKey) return

      const key = e.key

      // Number keys 1-9: select pane by position (mode-aware)
      if (key >= '1' && key <= '9') {
        const index = Number.parseInt(key, 10) - 1
        const visibleSessions = opts.sessions.filter((s) => !store.hiddenPaneIds.has(s.id))
        if (index < visibleSessions.length) {
          const session = visibleSessions[index]
          if (opts.layoutMode === 'custom' && opts.dockviewApi) {
            const panel = opts.dockviewApi.getPanel(session.id)
            if (panel) {
              panel.focus()
            }
          } else {
            store.selectPane(session.id)
          }
          e.preventDefault()
        }
        return
      }

      switch (key) {
        case 'Enter': {
          if (store.selectedPaneId) {
            store.expandPane(store.selectedPaneId)
            e.preventDefault()
          }
          break
        }

        case 'p': {
          const selectedId = store.selectedPaneId
          if (selectedId) {
            if (store.pinnedPaneIds.has(selectedId)) {
              store.unpinPane(selectedId)
            } else {
              store.pinPane(selectedId)
            }
            e.preventDefault()
          }
          break
        }

        case 'h': {
          const selectedId = store.selectedPaneId
          if (selectedId) {
            store.hidePane(selectedId)
            // Clear selection since the pane is now hidden
            store.selectPane(null)
            e.preventDefault()
          }
          break
        }

        case 'm': {
          store.setDisplayMode(store.displayMode === 'chat' ? 'developer' : 'chat')
          e.preventDefault()
          break
        }

        case '+':
        case '=': {
          const current = store.gridOverride
          const currentCols = current?.cols ?? 2
          if (currentCols < MAX_COLS) {
            const newCols = currentCols + 1
            // Keep rows proportional: same as current or compute from session count
            const currentRows = current?.rows ?? Math.ceil(opts.sessions.length / currentCols)
            store.setGridOverride({ cols: newCols, rows: currentRows })
          }
          e.preventDefault()
          break
        }

        case '-': {
          const current = store.gridOverride
          const currentCols = current?.cols ?? 2
          if (currentCols > MIN_COLS) {
            const newCols = currentCols - 1
            const currentRows = current?.rows ?? Math.ceil(opts.sessions.length / currentCols)
            store.setGridOverride({ cols: newCols, rows: currentRows })
          }
          e.preventDefault()
          break
        }
      }
    }

    document.addEventListener('keydown', handler)
    return () => document.removeEventListener('keydown', handler)
  }, [])
}
