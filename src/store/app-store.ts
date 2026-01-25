import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface AppState {
  // Search state
  searchQuery: string
  recentSearches: string[]
  isCommandPaletteOpen: boolean

  // Actions
  setSearchQuery: (query: string) => void
  addRecentSearch: (query: string) => void
  clearSearch: () => void
  openCommandPalette: () => void
  closeCommandPalette: () => void
  toggleCommandPalette: () => void
}

export const useAppStore = create<AppState>()(
  persist(
    (set) => ({
      searchQuery: '',
      recentSearches: [],
      isCommandPaletteOpen: false,

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
    }),
    {
      name: 'claude-view-storage',
      partialize: (state) => ({ recentSearches: state.recentSearches }),
    }
  )
)
