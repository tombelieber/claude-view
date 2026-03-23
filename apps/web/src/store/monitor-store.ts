import { create } from 'zustand'
import { type StorageValue, persist } from 'zustand/middleware'
import type { ActionCategory } from '../components/live/action-log/types'

export type VerboseFilter = ActionCategory[] | 'all'

export type DisplayMode = 'chat' | 'developer'

interface MonitorState {
  // Grid layout
  gridOverride: { cols: number; rows: number } | null
  compactHeaders: boolean

  // Pane state
  selectedPaneId: string | null
  expandedPaneId: string | null
  pinnedPaneIds: Set<string>
  hiddenPaneIds: Set<string>
  /** @deprecated Use `displayMode` instead. Kept for files pending deletion. */
  verboseMode: boolean
  verboseFilter: VerboseFilter
  richRenderMode: 'rich' | 'json'
  displayMode: DisplayMode

  // Actions
  setGridOverride: (override: { cols: number; rows: number } | null) => void
  setCompactHeaders: (compact: boolean) => void
  selectPane: (id: string | null) => void
  expandPane: (id: string | null) => void
  pinPane: (id: string) => void
  unpinPane: (id: string) => void
  hidePane: (id: string) => void
  showPane: (id: string) => void
  /** @deprecated Use `setDisplayMode` instead. Kept for files pending deletion. */
  toggleVerbose: () => void
  setVerboseFilter: (category: ActionCategory | 'all') => void
  setRichRenderMode: (mode: 'rich' | 'json') => void
  setDisplayMode: (mode: DisplayMode) => void
}

export const useMonitorStore = create<MonitorState>()(
  persist(
    (set, get) => ({
      gridOverride: null,
      compactHeaders: false,
      selectedPaneId: null,
      expandedPaneId: null,
      pinnedPaneIds: new Set<string>(),
      hiddenPaneIds: new Set<string>(),
      verboseMode: false,
      verboseFilter: 'all' as VerboseFilter,
      richRenderMode: 'rich',
      displayMode: 'chat' as DisplayMode,

      setGridOverride: (override) => set({ gridOverride: override }),
      setCompactHeaders: (compact) => set({ compactHeaders: compact }),

      selectPane: (id) => set({ selectedPaneId: id }),
      expandPane: (id) => set({ expandedPaneId: id }),

      pinPane: (id) =>
        set((state) => {
          const next = new Set(state.pinnedPaneIds)
          next.add(id)
          return { pinnedPaneIds: next }
        }),

      unpinPane: (id) =>
        set((state) => {
          const next = new Set(state.pinnedPaneIds)
          next.delete(id)
          return { pinnedPaneIds: next }
        }),

      hidePane: (id) =>
        set((state) => {
          const next = new Set(state.hiddenPaneIds)
          next.add(id)
          return { hiddenPaneIds: next }
        }),

      showPane: (id) =>
        set((state) => {
          const next = new Set(state.hiddenPaneIds)
          next.delete(id)
          return { hiddenPaneIds: next }
        }),

      toggleVerbose: () =>
        set((state) => ({
          verboseMode: !state.verboseMode,
          displayMode: state.verboseMode ? 'chat' : 'developer',
        })),
      setDisplayMode: (mode) => set({ displayMode: mode, verboseMode: mode === 'developer' }),
      setVerboseFilter: (category) => {
        if (category === 'all') {
          set({ verboseFilter: 'all' })
          return
        }
        const current = get().verboseFilter
        // From "all" → start fresh with just this category
        if (current === 'all') {
          set({ verboseFilter: [category] })
          return
        }
        // Toggle: remove if present, add if absent
        const next = current.includes(category)
          ? current.filter((c) => c !== category)
          : [...current, category]
        // Empty → revert to all
        set({ verboseFilter: next.length === 0 ? 'all' : next })
      },
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
        verboseFilter: state.verboseFilter,
        richRenderMode: state.richRenderMode,
        displayMode: state.displayMode,
      }),
      storage: {
        getItem: (name: string): StorageValue<Partial<MonitorState>> | null => {
          const str = localStorage.getItem(name)
          if (!str) return null
          let parsed: StorageValue<Record<string, unknown>>
          try {
            parsed = JSON.parse(str) as StorageValue<Record<string, unknown>>
          } catch {
            console.error('monitor-store: corrupt localStorage, resetting')
            return null
          }
          // Convert arrays back to Sets after deserialization
          if (parsed.state) {
            if (Array.isArray(parsed.state.pinnedPaneIds)) {
              parsed.state.pinnedPaneIds = new Set(parsed.state.pinnedPaneIds as string[])
            }
            if (Array.isArray(parsed.state.hiddenPaneIds)) {
              parsed.state.hiddenPaneIds = new Set(parsed.state.hiddenPaneIds as string[])
            }
            // Migrate old verboseMode → displayMode
            if (
              parsed.state.displayMode === undefined &&
              typeof parsed.state.verboseMode === 'boolean'
            ) {
              parsed.state.displayMode = parsed.state.verboseMode ? 'developer' : 'chat'
            }
            // Migrate old single-string verboseFilter to new format
            const vf = parsed.state.verboseFilter
            if (typeof vf === 'string' && vf !== 'all') {
              parsed.state.verboseFilter = [vf] as unknown as VerboseFilter
            } else if (Array.isArray(vf) && vf.length === 0) {
              parsed.state.verboseFilter = 'all'
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
              toStore.state.pinnedPaneIds = [
                ...toStore.state.pinnedPaneIds,
              ] as unknown as Set<string>
            }
            if (toStore.state.hiddenPaneIds instanceof Set) {
              toStore.state.hiddenPaneIds = [
                ...toStore.state.hiddenPaneIds,
              ] as unknown as Set<string>
            }
          }
          localStorage.setItem(name, JSON.stringify(toStore))
        },
        removeItem: (name: string) => localStorage.removeItem(name),
      },
    },
  ),
)
