import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { SerializedDockview } from 'dockview-react'

// ---------------------------------------------------------------------------
// Legacy migration: move scattered localStorage keys into the unified store.
// Runs once on first import — follows the same pattern as monitor-store.ts.
// ---------------------------------------------------------------------------
try {
  if (!localStorage.getItem('claude-view:dock-layouts')) {
    const chatRaw = localStorage.getItem('claude-view:chat-layout')
    const monitorRaw = localStorage.getItem('claude-view:monitor-layout')
    const modeRaw = localStorage.getItem('claude-view:monitor-layout-mode')
    const presetsRaw = localStorage.getItem('claude-view:monitor-presets')

    if (chatRaw || monitorRaw || modeRaw || presetsRaw) {
      const migrated = {
        state: {
          chatLayout: chatRaw ? JSON.parse(chatRaw) : null,
          monitorLayout: monitorRaw ? JSON.parse(monitorRaw) : null,
          monitorLayoutMode: modeRaw === 'custom' ? 'custom' : 'auto-grid',
          monitorPresets: presetsRaw ? JSON.parse(presetsRaw) : {},
        },
        version: 1,
      }
      localStorage.setItem('claude-view:dock-layouts', JSON.stringify(migrated))
      localStorage.removeItem('claude-view:chat-layout')
      localStorage.removeItem('claude-view:monitor-layout')
      localStorage.removeItem('claude-view:monitor-layout-mode')
      localStorage.removeItem('claude-view:monitor-presets')
    }
  }
} catch {
  /* corrupt localStorage — store will start fresh */
}

// ---------------------------------------------------------------------------
// CLI panel stripping — tmux terminals don't survive reloads, so we strip
// them at SAVE time. Restored layouts never contain CLI panels.
// ---------------------------------------------------------------------------

function stripCliPanels(layout: SerializedDockview): SerializedDockview {
  const cleaned = structuredClone(layout)
  if (cleaned.panels) {
    for (const key of Object.keys(cleaned.panels ?? {})) {
      if (key.startsWith('chat-cli-')) delete cleaned.panels[key]
    }
  }
  if (cleaned.grid?.root) stripCliPanelRefs(cleaned.grid.root as unknown as Record<string, unknown>)
  return cleaned
}

function stripCliPanelRefs(node: Record<string, unknown>): void {
  if (Array.isArray(node.data)) {
    node.data = (node.data as Array<Record<string, unknown>>).filter((child) => {
      if (typeof child.id === 'string' && child.id.startsWith('chat-cli-')) return false
      if (child.data) stripCliPanelRefs(child)
      return true
    })
  }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export type LayoutMode = 'auto-grid' | 'custom'

interface DockLayoutState {
  // Persisted
  chatLayout: SerializedDockview | null
  monitorLayout: SerializedDockview | null
  monitorLayoutMode: LayoutMode
  monitorPresets: Record<string, SerializedDockview>

  // Transient (not persisted)
  activePreset: string | null

  // Actions
  saveChatLayout: (layout: SerializedDockview) => void
  saveMonitorLayout: (layout: SerializedDockview) => void
  setMonitorLayoutMode: (mode: LayoutMode) => void
  toggleMonitorLayoutMode: () => void
  clearMonitorLayout: () => void
  setActivePreset: (name: string | null) => void
  savePreset: (name: string, layout: SerializedDockview) => void
  deletePreset: (name: string) => void
}

export const useDockLayoutStore = create<DockLayoutState>()(
  persist(
    (set) => ({
      chatLayout: null,
      monitorLayout: null,
      monitorLayoutMode: 'auto-grid' as LayoutMode,
      monitorPresets: {},
      activePreset: null,

      saveChatLayout: (layout) => set({ chatLayout: stripCliPanels(layout) }),
      saveMonitorLayout: (layout) => set({ monitorLayout: layout, activePreset: null }),
      setMonitorLayoutMode: (mode) => set({ monitorLayoutMode: mode }),
      toggleMonitorLayoutMode: () =>
        set((s) => ({
          monitorLayoutMode: s.monitorLayoutMode === 'auto-grid' ? 'custom' : 'auto-grid',
        })),
      clearMonitorLayout: () => set({ monitorLayout: null }),
      setActivePreset: (name) => set({ activePreset: name }),
      savePreset: (name, layout) =>
        set((s) => ({ monitorPresets: { ...s.monitorPresets, [name]: layout } })),
      deletePreset: (name) =>
        set((s) => {
          const next = { ...s.monitorPresets }
          delete next[name]
          return { monitorPresets: next }
        }),
    }),
    {
      name: 'claude-view:dock-layouts',
      version: 1,
      partialize: (state) => ({
        chatLayout: state.chatLayout,
        monitorLayout: state.monitorLayout,
        monitorLayoutMode: state.monitorLayoutMode,
        monitorPresets: state.monitorPresets,
      }),
    },
  ),
)
