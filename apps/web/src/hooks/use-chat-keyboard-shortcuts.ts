import type { DockviewApi } from 'dockview-react'
import { useEffect } from 'react'

/**
 * Global keyboard shortcuts for the chat dockview layout.
 *
 * | Shortcut           | Action                  |
 * |--------------------|-------------------------|
 * | Ctrl+T             | New session (new tab)   |
 * | Ctrl+W             | Close active tab        |
 * | Ctrl+\             | Split active tab right  |
 * | Ctrl+Shift+\       | Split active tab down   |
 * | Ctrl+Tab           | Next tab in group       |
 * | Ctrl+Shift+Tab     | Previous tab in group   |
 */
export function useChatKeyboardShortcuts(api: DockviewApi | null) {
  useEffect(() => {
    if (!api) return

    // Capture non-null api in a local const so TS narrows inside the closure
    const dv: DockviewApi = api

    function handleKeyDown(e: KeyboardEvent) {
      // Ignore when typing in inputs/textareas
      const tag = (e.target as HTMLElement)?.tagName
      if (tag === 'INPUT' || tag === 'TEXTAREA' || (e.target as HTMLElement)?.isContentEditable) {
        return
      }

      const ctrl = e.ctrlKey || e.metaKey

      // Ctrl+T — New session tab
      if (ctrl && !e.shiftKey && e.key === 't') {
        e.preventDefault()
        const id = `chat-new-${Date.now()}`
        dv.addPanel({
          id,
          component: 'chat',
          title: 'New Session',
          params: { sessionId: '' },
        })
        return
      }

      // Ctrl+W — Close active tab
      if (ctrl && !e.shiftKey && e.key === 'w') {
        e.preventDefault()
        const active = dv.activePanel
        if (active) {
          active.api.close()
        }
        return
      }

      // Ctrl+\ — Split right
      if (ctrl && !e.shiftKey && e.key === '\\') {
        e.preventDefault()
        const active = dv.activePanel
        if (active) {
          const sessionId = (active.params as { sessionId?: string })?.sessionId
          if (sessionId) {
            dv.addPanel({
              id: `chat-${sessionId}-split-r-${Date.now()}`,
              component: 'chat',
              title: active.title ?? sessionId.slice(0, 8),
              params: { sessionId, isWatching: true },
              position: { referencePanel: active.id, direction: 'right' },
            })
          }
        }
        return
      }

      // Ctrl+Shift+\ — Split down
      if (ctrl && e.shiftKey && e.key === '\\') {
        e.preventDefault()
        const active = dv.activePanel
        if (active) {
          const sessionId = (active.params as { sessionId?: string })?.sessionId
          if (sessionId) {
            dv.addPanel({
              id: `chat-${sessionId}-split-d-${Date.now()}`,
              component: 'chat',
              title: active.title ?? sessionId.slice(0, 8),
              params: { sessionId, isWatching: true },
              position: { referencePanel: active.id, direction: 'below' },
            })
          }
        }
        return
      }

      // Ctrl+Tab — Next tab in group
      if (ctrl && !e.shiftKey && e.key === 'Tab') {
        e.preventDefault()
        const active = dv.activePanel
        if (!active) return
        const group = active.group
        if (!group) return
        const panels = group.panels
        const idx = panels.indexOf(active)
        const next = panels[(idx + 1) % panels.length]
        if (next) next.api.setActive()
        return
      }

      // Ctrl+Shift+Tab — Previous tab in group
      if (ctrl && e.shiftKey && e.key === 'Tab') {
        e.preventDefault()
        const active = dv.activePanel
        if (!active) return
        const group = active.group
        if (!group) return
        const panels = group.panels
        const idx = panels.indexOf(active)
        const prev = panels[(idx - 1 + panels.length) % panels.length]
        if (prev) prev.api.setActive()
        return
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [api])
}
