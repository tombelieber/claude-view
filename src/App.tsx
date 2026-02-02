import { useEffect } from 'react'
import { Outlet, useNavigate, useLocation } from 'react-router-dom'
import { FolderOpen } from 'lucide-react'
import { useProjectSummaries } from './hooks/use-projects'
import { useAppStore } from './store/app-store'
import { useTheme } from './hooks/use-theme'
import { Header } from './components/Header'
import { Sidebar } from './components/Sidebar'
import { StatusBar } from './components/StatusBar'
import { CommandPalette } from './components/CommandPalette'
import { DashboardSkeleton, ErrorState, EmptyState } from './components/LoadingStates'

export default function App() {
  const { data: summaries, isLoading, error, refetch } = useProjectSummaries()
  const { isCommandPaletteOpen, openCommandPalette, closeCommandPalette } = useAppStore()
  useTheme() // Apply dark class to <html>
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

  // Loading state - show skeleton instead of blank screen
  if (isLoading) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-950" role="status" aria-busy="true" aria-label="Loading application">
        <div className="h-14 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 animate-pulse" />
        <DashboardSkeleton />
      </div>
    )
  }

  // Error state with retry button
  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-950 flex items-center justify-center">
        <ErrorState
          message={error.message}
          onRetry={() => refetch()}
        />
      </div>
    )
  }

  // Empty state with descriptive text
  if (!summaries || summaries.length === 0) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-950 flex items-center justify-center">
        <EmptyState
          icon={<FolderOpen className="w-6 h-6 text-gray-400" />}
          title="No Claude Code sessions found"
          description="Start using Claude Code in your terminal to see your session history here. Sessions will appear after your first conversation."
          action={{
            label: 'Refresh',
            onClick: () => refetch(),
          }}
        />
      </div>
    )
  }

  return (
    <div className="h-screen flex flex-col bg-white dark:bg-gray-950">
      <a href="#main" className="skip-to-content">Skip to content</a>
      <Header />

      <div className="flex-1 flex overflow-hidden">
        <Sidebar projects={summaries} />

        <main id="main" className="flex-1 overflow-hidden bg-gray-50 dark:bg-gray-950">
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
