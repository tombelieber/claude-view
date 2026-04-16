import { create } from 'zustand'
import { type StorageValue, persist } from 'zustand/middleware'

// Migrate: strip pinnedPaneIds/hiddenPaneIds from persisted state (now in-memory only).
try {
  const stored = localStorage.getItem('claude-view:monitor-grid-prefs')
  if (stored) {
    const parsed = JSON.parse(stored)
    if (parsed?.state?.pinnedPaneIds || parsed?.state?.hiddenPaneIds) {
      parsed.state.pinnedPaneIds = undefined
      parsed.state.hiddenPaneIds = undefined
      localStorage.setItem('claude-view:monitor-grid-prefs', JSON.stringify(parsed))
    }
  }
} catch {
  /* ignore */
}
/** Inlined from action-log/types to break import dependency on soon-to-be-deleted directory. */
type ActionCategory =
  | 'skill'
  | 'mcp'
  | 'builtin'
  | 'agent'
  | 'hook'
  | 'hook_progress'
  | 'error'
  | 'system'
  | 'snapshot'
  | 'queue'

export type VerboseFilter = ActionCategory[] | 'all'

export type DisplayMode = 'chat' | 'developer'

export type DetailTabId =
  | 'overview'
  | 'chat'
  | 'sub-agents'
  | 'teams'
  | 'cost'
  | 'tasks'
  | 'changes'
  | 'plan'
  | 'cli'

interface MonitorState {
  // Grid layout
  gridOverride: { cols: number; rows: number } | null
  compactHeaders: boolean

  // Pane state
  selectedPaneId: string | null
  expandedPaneId: string | null
  pinnedPaneIds: Set<string>
  hiddenPaneIds: Set<string>
  verboseFilter: VerboseFilter
  richRenderMode: 'rich' | 'json'
  displayMode: DisplayMode
  /** Last tab the user clicked in the session detail panel — persisted across sessions. */
  preferredDetailTab: DetailTabId

  // Actions
  setGridOverride: (override: { cols: number; rows: number } | null) => void
  setCompactHeaders: (compact: boolean) => void
  selectPane: (id: string | null) => void
  expandPane: (id: string | null) => void
  pinPane: (id: string) => void
  unpinPane: (id: string) => void
  hidePane: (id: string) => void
  showPane: (id: string) => void
  setVerboseFilter: (category: ActionCategory | 'all') => void
  setRichRenderMode: (mode: 'rich' | 'json') => void
  setDisplayMode: (mode: DisplayMode) => void
  setPreferredDetailTab: (tab: DetailTabId) => void
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
      verboseFilter: 'all' as VerboseFilter,
      richRenderMode: 'rich',
      displayMode: 'chat' as DisplayMode,
      preferredDetailTab: 'overview' as DetailTabId,

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

      setDisplayMode: (mode) => set({ displayMode: mode }),
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
      setPreferredDetailTab: (tab) => set({ preferredDetailTab: tab }),
    }),
    {
      name: 'claude-view:monitor-grid-prefs',
      partialize: (state) => ({
        gridOverride: state.gridOverride,
        compactHeaders: state.compactHeaders,
        verboseFilter: state.verboseFilter,
        richRenderMode: state.richRenderMode,
        displayMode: state.displayMode,
        preferredDetailTab: state.preferredDetailTab,
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
          if (parsed.state) {
            // Migrate old verboseMode → displayMode (localStorage backward compat)
            if (
              parsed.state.displayMode === undefined &&
              typeof parsed.state.verboseMode === 'boolean'
            ) {
              parsed.state.displayMode = parsed.state.verboseMode ? 'developer' : 'chat'
            }
            // Clean up legacy verboseMode from persisted state
            parsed.state.verboseMode = undefined
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
          localStorage.setItem(name, JSON.stringify(value))
        },
        removeItem: (name: string) => localStorage.removeItem(name),
      },
    },
  ),
)
