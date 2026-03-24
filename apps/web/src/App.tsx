import { type CodeRenderContextValue, CodeRenderProvider } from '@claude-view/shared'
import { useEffect, useMemo } from 'react'
import { Outlet } from 'react-router-dom'
import { AuthBanner } from './components/AuthBanner'
import { CodeBlock } from './components/CodeBlock'
import { ColdStartOverlay } from './components/ColdStartOverlay'
import { CommandPalette } from './components/CommandPalette'
import { Header } from './components/Header'
import { ErrorState, LiveMonitorSkeleton } from './components/LoadingStates'
import { Sidebar } from './components/Sidebar'
import { StatusBar } from './components/StatusBar'
import { TelemetryBanner } from './components/TelemetryBanner'
import { useTelemetryPrompt } from './hooks/use-telemetry-prompt'
import { CompactCodeBlock } from './components/live/CompactCodeBlock'
import { useLiveSessions } from './components/live/use-live-sessions'
import { useIndexingProgress } from './hooks/use-indexing-progress'
import { useNotificationSound } from './hooks/use-notification-sound'
import { usePatternAlert } from './hooks/use-pattern-alert'
import { useProjectSummaries } from './hooks/use-projects'
import { useTelemetry } from './hooks/use-telemetry'
import { useTheme } from './hooks/use-theme'
import { useAppStore } from './store/app-store'
import { useLiveCommandStore } from './store/live-command-context'

export default function App() {
  const { data: summaries, isLoading, error, refetch } = useProjectSummaries()
  const {
    isCommandPaletteOpen,
    openCommandPalette,
    closeCommandPalette,
    sidebarCollapsed,
    toggleSidebar,
  } = useAppStore()
  useTheme() // Apply dark class to <html>
  const { enableTelemetry, disableTelemetry, isPending: isTelemetryPending } = useTelemetry()
  const { shouldPrompt } = useTelemetryPrompt()
  const indexingProgress = useIndexingProgress()
  const liveSessions = useLiveSessions()
  const {
    settings: soundSettings,
    updateSettings: updateSoundSettings,
    previewSound,
    audioUnlocked,
  } = useNotificationSound(liveSessions.sessions)
  const liveContext = useLiveCommandStore((s) => s.context)
  usePatternAlert()

  // Provide shiki-based code renderers to shared components via context
  const codeRenderValue = useMemo<CodeRenderContextValue>(
    () => ({ CodeBlock, CompactCodeBlock }),
    [],
  )

  // Global keyboard shortcut: Cmd+K
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        openCommandPalette()
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
        e.preventDefault()
        toggleSidebar()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [openCommandPalette, toggleSidebar])

  // Loading state - show live monitor skeleton (home page is mission control)
  if (isLoading) {
    return (
      <div
        className="min-h-screen bg-gray-50 dark:bg-gray-950"
        role="status"
        aria-busy="true"
        aria-label="Loading application"
      >
        <div className="h-14 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 animate-pulse" />
        <LiveMonitorSkeleton />
      </div>
    )
  }

  // Error state with retry button
  if (error) {
    return (
      <div className="min-h-screen bg-gray-50 dark:bg-gray-950 flex items-center justify-center">
        <ErrorState message={error.message} onRetry={() => refetch()} />
      </div>
    )
  }

  return (
    <CodeRenderProvider value={codeRenderValue}>
      <div className="h-screen flex flex-col bg-white dark:bg-gray-950">
        <a href="#main" className="skip-to-content">
          Skip to content
        </a>
        <Header
          soundSettings={soundSettings}
          onSoundSettingsChange={updateSoundSettings}
          onSoundPreview={previewSound}
          audioUnlocked={audioUnlocked}
        />
        {shouldPrompt && (
          <TelemetryBanner
            onEnable={enableTelemetry}
            onDisable={disableTelemetry}
            isPending={isTelemetryPending}
          />
        )}
        <AuthBanner />
        <ColdStartOverlay progress={indexingProgress} />

        <div className="flex-1 flex overflow-hidden">
          <Sidebar projects={summaries ?? []} collapsed={sidebarCollapsed} />

          <main id="main" className="flex-1 overflow-y-auto bg-gray-50 dark:bg-gray-950">
            <Outlet context={{ summaries: summaries ?? [], liveSessions, indexingProgress }} />
          </main>
        </div>

        <StatusBar projects={summaries ?? []} />

        <CommandPalette
          isOpen={isCommandPaletteOpen}
          onClose={closeCommandPalette}
          projects={summaries ?? []}
          liveContext={liveContext ?? undefined}
        />
      </div>
    </CodeRenderProvider>
  )
}
