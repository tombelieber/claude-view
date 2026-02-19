import { create } from 'zustand'
import { persist, type StorageValue } from 'zustand/middleware'

interface MonitorState {
  // Grid layout
  gridOverride: { cols: number; rows: number } | null
  compactHeaders: boolean

  // Pane state
  selectedPaneId: string | null
  expandedPaneId: string | null
  pinnedPaneIds: Set<string>
  hiddenPaneIds: Set<string>
  verboseMode: boolean
  richRenderMode: 'rich' | 'json'

  // Actions
  setGridOverride: (override: { cols: number; rows: number } | null) => void
  setCompactHeaders: (compact: boolean) => void
  selectPane: (id: string | null) => void
  expandPane: (id: string | null) => void
  pinPane: (id: string) => void
  unpinPane: (id: string) => void
  hidePane: (id: string) => void
  showPane: (id: string) => void
  toggleVerbose: () => void
  setRichRenderMode: (mode: 'rich' | 'json') => void
}

export const useMonitorStore = create<MonitorState>()(
  persist(
    (set) => ({
      gridOverride: null,
      compactHeaders: false,
      selectedPaneId: null,
      expandedPaneId: null,
      pinnedPaneIds: new Set<string>(),
      hiddenPaneIds: new Set<string>(),
      verboseMode: false,
      richRenderMode: 'rich',

      setGridOverride: (override) => set({ gridOverride: override }),
      setCompactHeaders: (compact) => set({ compactHeaders: compact }),

      selectPane: (id) => set({ selectedPaneId: id }),
      expandPane: (id) => set({ expandedPaneId: id }),

      pinPane: (id) => set((state) => {
        const next = new Set(state.pinnedPaneIds)
        next.add(id)
        return { pinnedPaneIds: next }
      }),

      unpinPane: (id) => set((state) => {
        const next = new Set(state.pinnedPaneIds)
        next.delete(id)
        return { pinnedPaneIds: next }
      }),

      hidePane: (id) => set((state) => {
        const next = new Set(state.hiddenPaneIds)
        next.add(id)
        return { hiddenPaneIds: next }
      }),

      showPane: (id) => set((state) => {
        const next = new Set(state.hiddenPaneIds)
        next.delete(id)
        return { hiddenPaneIds: next }
      }),

      toggleVerbose: () => set((state) => ({ verboseMode: !state.verboseMode })),
      setRichRenderMode: (mode) => set({ richRenderMode: mode }),
    }),
    {
      name: 'claude-view:monitor-grid-prefs',
      partialize: (state) => ({
        gridOverride: state.gridOverride,
        compactHeaders: state.compactHeaders,
        pinnedPaneIds: state.pinnedPaneIds,
        hiddenPaneIds: state.hiddenPaneIds,
        verboseMode: state.verboseMode,
        richRenderMode: state.richRenderMode,
      }),
      storage: {
        getItem: (name: string): StorageValue<Partial<MonitorState>> | null => {
          const str = localStorage.getItem(name)
          if (!str) return null
          const parsed = JSON.parse(str) as StorageValue<Record<string, unknown>>
          // Convert arrays back to Sets after deserialization
          if (parsed.state) {
            if (Array.isArray(parsed.state.pinnedPaneIds)) {
              parsed.state.pinnedPaneIds = new Set(parsed.state.pinnedPaneIds as string[])
            }
            if (Array.isArray(parsed.state.hiddenPaneIds)) {
              parsed.state.hiddenPaneIds = new Set(parsed.state.hiddenPaneIds as string[])
            }
          }
          return parsed as StorageValue<Partial<MonitorState>>
        },
        setItem: (name: string, value: StorageValue<Partial<MonitorState>>) => {
          // Convert Sets to arrays for JSON serialization
          const toStore = { ...value }
          if (toStore.state) {
            toStore.state = { ...toStore.state }
            if (toStore.state.pinnedPaneIds instanceof Set) {
              toStore.state.pinnedPaneIds = [...toStore.state.pinnedPaneIds] as unknown as Set<string>
            }
            if (toStore.state.hiddenPaneIds instanceof Set) {
              toStore.state.hiddenPaneIds = [...toStore.state.hiddenPaneIds] as unknown as Set<string>
            }
          }
          localStorage.setItem(name, JSON.stringify(toStore))
        },
        removeItem: (name: string) => localStorage.removeItem(name),
      },
    }
  )
)
