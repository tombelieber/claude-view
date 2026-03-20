import type { DockviewApi } from 'dockview-react'
import { useCallback, useEffect, useRef } from 'react'

const STORAGE_KEY = 'claude-view:chat-open-tabs'

export interface ChatTabState {
  /** Session IDs of open tabs, in order. Empty string = blank "New Chat" tab. */
  openTabs: string[]
  /** Session ID of the active (focused) tab, or null. */
  activeTab: string | null
}

/** Read persisted tab state from localStorage. */
export function readPersistedTabs(): ChatTabState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      if (Array.isArray(parsed.openTabs)) return parsed as ChatTabState
    }
  } catch {
    // Corrupt or unavailable — start fresh
  }
  return { openTabs: [], activeTab: null }
}

/** Write tab state to localStorage (debounced externally). */
function persistTabs(state: ChatTabState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
  } catch {
    // QuotaExceeded — state still works in-memory for this session
  }
}

/** Snapshot current dockview state into a ChatTabState. */
function snapshotTabs(api: DockviewApi): ChatTabState {
  const openTabs: string[] = []
  let activeTab: string | null = null

  for (const panel of api.panels) {
    const sid = (panel.params as { sessionId?: string })?.sessionId
    if (sid == null) continue
    openTabs.push(sid)
    if (panel.api.isActive) activeTab = sid
  }

  return { openTabs, activeTab }
}

/**
 * Persists open chat tab IDs + active tab to localStorage.
 * Listens for dockview structural changes and debounce-saves.
 */
export function useChatTabPersistence(api: DockviewApi | null): void {
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined)

  const save = useCallback(() => {
    if (!api) return
    if (debounceRef.current) clearTimeout(debounceRef.current)
    debounceRef.current = setTimeout(() => {
      persistTabs(snapshotTabs(api))
    }, 150)
  }, [api])

  useEffect(() => {
    if (!api) return

    // Save on any structural change
    const disposables = [
      api.onDidAddPanel(save),
      api.onDidRemovePanel(save),
      api.onDidActivePanelChange(save),
    ]

    return () => {
      for (const d of disposables) d.dispose()
      if (debounceRef.current) clearTimeout(debounceRef.current)
    }
  }, [api, save])
}
