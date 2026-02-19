import { create } from 'zustand'
import type { LiveSession } from '../components/live/use-live-sessions'
import type { LiveViewMode } from '../components/live/types'
import type { LiveSortField } from '../components/live/live-filter'

export interface LiveCommandContext {
  sessions: LiveSession[]
  viewMode: LiveViewMode
  onViewModeChange: (mode: LiveViewMode) => void
  onFilterStatus: (statuses: string[]) => void
  onClearFilters: () => void
  onSort: (field: LiveSortField) => void
  onSelectSession: (id: string) => void
  onToggleHelp: () => void
}

interface LiveCommandStore {
  context: LiveCommandContext | null
  setContext: (context: LiveCommandContext | null) => void
}

export const useLiveCommandStore = create<LiveCommandStore>((set) => ({
  context: null,
  setContext: (context) => set({ context }),
}))
