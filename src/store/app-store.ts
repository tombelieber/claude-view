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

  // Actions
  setSearchQuery: (query: string) => void
  addRecentSearch: (query: string) => void
  clearSearch: () => void
  openCommandPalette: () => void
  closeCommandPalette: () => void
  toggleCommandPalette: () => void
  setTheme: (theme: Theme) => void
  cycleTheme: () => void
}

const THEME_CYCLE: Theme[] = ['light', 'dark', 'system']

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      searchQuery: '',
      recentSearches: [],
      isCommandPaletteOpen: false,
      theme: 'system',

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

      setTheme: (theme) => set({ theme }),
      cycleTheme: () => set((state) => {
        const idx = THEME_CYCLE.indexOf(state.theme)
        return { theme: THEME_CYCLE[(idx + 1) % THEME_CYCLE.length] }
      }),
    }),
    {
      name: 'claude-view-storage',
      partialize: (state) => ({
        recentSearches: state.recentSearches,
        theme: state.theme,
      }),
    }
  )
)
