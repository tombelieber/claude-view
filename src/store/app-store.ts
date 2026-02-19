import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export type Theme = 'light' | 'dark' | 'system'

interface AppState {
  // Search state
  searchQuery: string
  recentSearches: string[]
  isCommandPaletteOpen: boolean

  // Theme state
  theme: Theme

  // Sidebar state
  sidebarCollapsed: boolean

  // Live Monitor
  recentLiveCommands: string[]

  // Actions
  setSearchQuery: (query: string) => void
  addRecentSearch: (query: string) => void
  clearSearch: () => void
  openCommandPalette: () => void
  closeCommandPalette: () => void
  toggleCommandPalette: () => void
  setTheme: (theme: Theme) => void
  cycleTheme: () => void
  toggleSidebar: () => void
  addRecentLiveCommand: (id: string) => void
}

const THEME_CYCLE: Theme[] = ['light', 'dark', 'system']

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      searchQuery: '',
      recentSearches: [],
      isCommandPaletteOpen: false,
      theme: 'system',
      sidebarCollapsed: false,
      recentLiveCommands: [],

      setSearchQuery: (query) => set({ searchQuery: query }),

      addRecentSearch: (query) => set((state) => ({
        recentSearches: [
          query,
          ...state.recentSearches.filter(s => s !== query)
        ].slice(0, 10)
      })),

      clearSearch: () => set({ searchQuery: '' }),

      openCommandPalette: () => set({ isCommandPaletteOpen: true }),
      closeCommandPalette: () => set({ isCommandPaletteOpen: false }),
      toggleCommandPalette: () => set((state) => ({
        isCommandPaletteOpen: !state.isCommandPaletteOpen
      })),

      addRecentLiveCommand: (id) => set((state) => ({
        recentLiveCommands: [
          id,
          ...state.recentLiveCommands.filter(c => c !== id)
        ].slice(0, 5)
      })),

      setTheme: (theme) => set({ theme }),
      cycleTheme: () => set((state) => {
        const idx = THEME_CYCLE.indexOf(state.theme)
        return { theme: THEME_CYCLE[(idx + 1) % THEME_CYCLE.length] }
      }),

      toggleSidebar: () => set((state) => ({
        sidebarCollapsed: !state.sidebarCollapsed
      })),
    }),
    {
      name: 'claude-view-storage',
      partialize: (state) => ({
        recentSearches: state.recentSearches,
        recentLiveCommands: state.recentLiveCommands,
        theme: state.theme,
        sidebarCollapsed: state.sidebarCollapsed,
      }),
    }
  )
)
