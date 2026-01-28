import { useEffect } from 'react'
import { Outlet, useNavigate, useLocation } from 'react-router-dom'
import { Loader2, FolderOpen } from 'lucide-react'
import { useProjectSummaries } from './hooks/use-projects'
import { useAppStore } from './store/app-store'
import { Header } from './components/Header'
import { Sidebar } from './components/Sidebar'
import { StatusBar } from './components/StatusBar'
import { CommandPalette } from './components/CommandPalette'

export default function App() {
  const { data: summaries, isLoading, error } = useProjectSummaries()
  const { isCommandPaletteOpen, openCommandPalette, closeCommandPalette } = useAppStore()
  const navigate = useNavigate()
  const location = useLocation()

  // Global keyboard shortcut: Cmd+K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        openCommandPalette()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [openCommandPalette])

  // Loading state
  if (isLoading) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="flex items-center gap-3 text-gray-600">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Loading sessions...</span>
        </div>
      </div>
    )
  }

  // Error state
  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-red-600">
          <p className="font-medium">Failed to load projects</p>
          <p className="text-sm mt-1">{error.message}</p>
        </div>
      </div>
    )
  }

  // Empty state
  if (!summaries || summaries.length === 0) {
    return (
      <div className="min-h-screen bg-gray-50 flex items-center justify-center">
        <div className="text-center text-gray-500">
          <FolderOpen className="w-12 h-12 mx-auto mb-4 text-gray-300" />
          <p className="font-medium">No Claude Code sessions found</p>
          <p className="text-sm mt-1">Start using Claude Code to see your sessions here</p>
        </div>
      </div>
    )
  }

  return (
    <div className="h-screen flex flex-col">
      <a href="#main" className="skip-to-content">Skip to content</a>
      <Header />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar projects={summaries} />

        <main id="main" className="flex-1 overflow-hidden bg-gray-50">
          <Outlet context={{ summaries }} />
        </main>
      </div>

      <StatusBar projects={summaries} />

      <CommandPalette
        isOpen={isCommandPaletteOpen}
        onClose={closeCommandPalette}
        projects={summaries}
      />
    </div>
  )
}
