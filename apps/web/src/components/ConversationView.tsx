import type { PermissionMode } from '../types/control'

const VALID_MODES: PermissionMode[] = [
  'default',
  'acceptEdits',
  'bypassPermissions',
  'plan',
  'dontAsk',
]
import { FindProvider } from '@claude-view/shared/contexts/FindContext'
import {
  ArrowLeft,
  ChevronDown,
  Copy,
  Download,
  FileX,
  MessageSquare,
  PanelRight,
  Terminal,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link, useNavigate, useOutletContext, useParams, useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import { ExpandProvider } from '../contexts/ExpandContext'
import { ThreadHighlightProvider } from '../contexts/ThreadHighlightContext'
import { useConversation } from '../hooks/use-conversation'
import { useHookEvents } from '../hooks/use-hook-events'
import { useProjectSessions } from '../hooks/use-projects'
import type { ProjectSummary } from '../hooks/use-projects'
import { useRichSessionData } from '../hooks/use-rich-session-data'
import { isNotFoundError, useSession } from '../hooks/use-session'
import { useSessionDetail } from '../hooks/use-session-detail'
import { computeCategoryCounts } from '../lib/compute-category-counts'
import { deriveInputBarState } from '../lib/control-status-map'
import {
  type ExportMetadata,
  downloadHtml,
  exportToPdf,
  generateStandaloneHtml,
} from '../lib/export-html'
import { copyToClipboard, downloadMarkdown, generateMarkdown } from '../lib/export-markdown'
import { hookEventsToRichMessages, mergeByTimestamp } from '../lib/hook-events-to-messages'
import { messagesToRichMessages } from '../lib/message-to-rich'
import { getContextLimit } from '../lib/model-context-windows'
import { TOAST_DURATION } from '../lib/notify'
import { cn } from '../lib/utils'
import { useMonitorStore } from '../store/monitor-store'
import type { ConnectionHealth } from '../types/control'
import { CommitsPanel } from './CommitsPanel'
import { ErrorBoundary } from './ErrorBoundary'
import { FilesTouchedPanel, buildFilesTouched } from './FilesTouchedPanel'
import { EmptyState, ErrorState, Skeleton } from './LoadingStates'
import { SearchInput } from './SearchInput'
import { SessionMetricsBar } from './SessionMetricsBar'
import { ShareModal } from './ShareModal'
import { ChatInputBar } from './chat/ChatInputBar'
import { ConnectionBanner } from './chat/ConnectionBanner'
import { ConversationThread } from './conversation/ConversationThread'
import { chatRegistry } from './conversation/blocks/chat/registry'
import { developerRegistry } from './conversation/blocks/developer/registry'
import { RichPane } from './live/RichPane'
import { SessionDetailPanel } from './live/SessionDetailPanel'
import { ViewModeControls } from './live/ViewModeControls'
import { historyToPanelData } from './live/session-panel-data'

/** Map sessionState to connection health for banner */
function deriveConnectionHealth(sessionState: string): ConnectionHealth {
  if (sessionState === 'reconnecting') return 'degraded'
  if (sessionState === 'error') return 'lost'
  return 'ok'
}

/** RichPane wrapper that reads verboseMode from the store (same as terminal view) */
function HistoryRichPane({
  messages,
  categoryCounts,
}: {
  messages: import('./live/RichPane').RichMessage[]
  categoryCounts?: import('../lib/compute-category-counts').CategoryCounts
}) {
  const verboseMode = useMonitorStore((s) => s.verboseMode)
  return (
    <RichPane
      messages={messages}
      isVisible={true}
      verboseMode={verboseMode}
      bufferDone={true}
      categoryCounts={categoryCounts}
    />
  )
}

export function ConversationView() {
  const { sessionId } = useParams()
  const navigate = useNavigate()
  const { summaries } = useOutletContext<{ summaries: ProjectSummary[] }>()

  // Session metadata
  const { data: sessionDetail, error: detailError } = useSessionDetail(sessionId || null)
  const projectDir = sessionDetail?.project ?? ''
  const project = summaries.find((p) => p.name === projectDir)
  const projectName = project?.displayName || projectDir

  // Full session for export (loads in background)
  const { data: session, error: sessionError } = useSession(sessionId || null)

  // Rich data + hook events for the side panel
  const { data: sessionsPage } = useProjectSessions(projectDir || undefined, { limit: 500 })
  const sessionInfo = sessionsPage?.sessions.find((s) => s.id === sessionId)
  const { data: richData } = useRichSessionData(sessionId || null)
  const hookEvents = useHookEvents(sessionId ?? '', !!sessionId)

  // Unified conversation hook: blocks + actions + session state
  const { blocks, actions, sessionInfo: convInfo } = useConversation(sessionId)
  const { isLive, sessionState } = convInfo

  // Detect missing JSONL (session in DB but file deleted)
  const isFileGone = !!sessionDetail && isNotFoundError(sessionError)

  // Loading: gate on sessionDetail only (blocks arrive async from new hook)
  const isLoading = isFileGone ? false : !sessionDetail && !detailError

  // Context gauge seeded from DB (live updates not available from new hook)
  const contextWindow = getContextLimit(
    sessionDetail?.primaryModel,
    sessionDetail?.totalInputTokens ?? undefined,
  )
  const contextPercent = sessionDetail?.totalInputTokens
    ? Math.round((sessionDetail.totalInputTokens / contextWindow) * 100)
    : 0

  // Mode state for ChatInputBar
  const [chatMode, setChatMode] = useState<PermissionMode>(() => {
    if (!sessionId) return 'default'
    try {
      const stored = localStorage.getItem(`claude-view:mode:${sessionId}`)
      return stored && VALID_MODES.includes(stored as PermissionMode)
        ? (stored as PermissionMode)
        : 'default'
    } catch {
      return 'default'
    }
  })
  const [chatModel, setChatModel] = useState('claude-sonnet-4-6')

  useEffect(() => {
    if (sessionDetail?.primaryModel) setChatModel(sessionDetail.primaryModel)
  }, [sessionDetail?.primaryModel])

  const handleModeChange = useCallback(
    (mode: PermissionMode) => {
      setChatMode(mode)
      if (sessionId) localStorage.setItem(`claude-view:mode:${sessionId}`, mode)
      actions.setPermissionMode(mode)
    },
    [sessionId, actions],
  )

  // Push persisted mode once session goes live
  const lastSentModeRef = useRef<PermissionMode | null>(null)
  useEffect(() => {
    if (isLive && chatMode !== 'default' && lastSentModeRef.current !== chatMode) {
      lastSentModeRef.current = chatMode
      actions.setPermissionMode(chatMode)
    }
  }, [isLive, chatMode, actions])

  const verboseMode = useMonitorStore((s) => s.verboseMode)
  const [exportMenuOpen, setExportMenuOpen] = useState(false)
  const exportMenuRef = useRef<HTMLDivElement>(null)
  const [resumeMenuOpen, setResumeMenuOpen] = useState(false)
  const resumeMenuRef = useRef<HTMLDivElement>(null)
  const [searchParams] = useSearchParams()

  // In-session find (Cmd+F / Ctrl+F)
  const [findOpen, setFindOpen] = useState(false)
  const [findQuery, setFindQuery] = useState('')
  const findOpenRef = useRef(findOpen)
  useEffect(() => {
    findOpenRef.current = findOpen
  }, [findOpen])

  const backUrl = useMemo(() => {
    const preserved = new URLSearchParams()
    const proj = searchParams.get('project')
    const branch = searchParams.get('branch')
    if (proj) preserved.set('project', proj)
    if (branch) preserved.set('branch', branch)
    const qs = preserved.toString()
    return qs ? `/sessions?${qs}` : '/sessions'
  }, [searchParams])

  const exportsReady = !!session

  const exportMeta: ExportMetadata | undefined = useMemo(() => {
    if (!sessionDetail) return undefined
    return {
      sessionId: sessionId || '',
      projectName,
      projectPath: sessionDetail.projectPath,
      primaryModel: sessionDetail.primaryModel,
      durationSeconds: sessionDetail.durationSeconds,
      totalInputTokens: sessionDetail.totalInputTokens,
      totalOutputTokens: sessionDetail.totalOutputTokens,
      messageCount: session?.messages?.length ?? 0,
      userPromptCount: sessionDetail.userPromptCount,
      toolCallCount: sessionDetail.toolCallCount,
      filesEditedCount: sessionDetail.filesEditedCount,
      filesReadCount: sessionDetail.filesReadCount,
      commitCount: sessionDetail.commitCount,
      gitBranch: sessionDetail.gitBranch,
      exportDate: new Date().toISOString(),
    }
  }, [sessionDetail, sessionId, projectName, session])

  const handleExportHtml = useCallback(() => {
    if (!session) return
    const html = generateStandaloneHtml(session.messages, exportMeta)
    downloadHtml(html, `conversation-${sessionId}.html`)
  }, [session, sessionId, exportMeta])

  const handleExportPdf = useCallback(() => {
    if (!session) return
    exportToPdf(session.messages, exportMeta)
  }, [session, exportMeta])

  const handleExportMarkdown = useCallback(() => {
    if (!session) return
    const markdown = generateMarkdown(session.messages, projectName, sessionId)
    downloadMarkdown(markdown, `conversation-${sessionId}.md`)
  }, [session, projectName, sessionId])

  const handleCopyMarkdown = useCallback(async () => {
    if (!session) return
    const markdown = generateMarkdown(session.messages, projectName, sessionId)
    const ok = await copyToClipboard(markdown)
    if (ok) {
      toast.success('Markdown copied to clipboard', { duration: TOAST_DURATION.micro })
    } else {
      toast.error('Copy failed', { description: 'Check browser permissions' })
    }
  }, [session, projectName, sessionId])

  const handleResume = useCallback(async () => {
    const projectPath = sessionDetail?.projectPath
    if (projectPath) {
      try {
        const res = await fetch(`/api/check-path?path=${encodeURIComponent(projectPath)}`)
        const data = await res.json()
        if (!data.exists) {
          toast.error('Project path unavailable', { description: 'Worktree may have been removed' })
          return
        }
      } catch {
        // Proceed anyway if check fails
      }
    }
    const cmd = `claude --resume ${sessionId}`
    const ok = await copyToClipboard(cmd)
    if (ok) {
      toast.success('Resume command copied — paste in terminal', { duration: TOAST_DURATION.micro })
    } else {
      toast.error('Copy failed', { description: 'Check browser permissions' })
    }
  }, [sessionId, sessionDetail])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      const modifierKey = e.metaKey || e.ctrlKey
      if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'e') {
        e.preventDefault()
        handleExportHtml()
      } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault()
        handleExportPdf()
      } else if (modifierKey && e.shiftKey && e.key.toLowerCase() === 'r') {
        e.preventDefault()
        handleResume()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleExportHtml, handleExportPdf, handleResume])

  // Cmd+F / Escape for find bar
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
        e.preventDefault()
        setFindOpen(true)
      }
      if (e.key === 'Escape' && findOpenRef.current) {
        setFindOpen(false)
        setFindQuery('')
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  // Close export menu on outside click
  useEffect(() => {
    if (!exportMenuOpen) return
    function handleClick(e: MouseEvent) {
      if (exportMenuRef.current && !exportMenuRef.current.contains(e.target as Node)) {
        setExportMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [exportMenuOpen])

  // Close resume menu on outside click
  useEffect(() => {
    if (!resumeMenuOpen) return
    function handleClick(e: MouseEvent) {
      if (resumeMenuRef.current && !resumeMenuRef.current.contains(e.target as Node)) {
        setResumeMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [resumeMenuOpen])

  const [panelOpen, setPanelOpen] = useState(true)

  // Verbose mode: rich messages for RichPane (developer terminal view)
  const richMessages = useMemo(
    () => (session?.messages?.length ? messagesToRichMessages(session.messages) : []),
    [session?.messages],
  )
  const richHookMessages = useMemo(() => hookEventsToRichMessages(hookEvents), [hookEvents])
  const richMessagesWithHookEvents = useMemo(() => {
    if (
      richHookMessages.length === 0 ||
      richMessages.some((m) => m.metadata?.type === 'hook_event')
    ) {
      return richMessages
    }
    return mergeByTimestamp(richMessages, richHookMessages, (m) => m.ts)
  }, [richMessages, richHookMessages])

  const historyCategoryCounts = useMemo(
    () => computeCategoryCounts(richMessagesWithHookEvents),
    [richMessagesWithHookEvents],
  )

  // Side panel data
  const panelData = useMemo(() => {
    if (!sessionDetail) return undefined
    return historyToPanelData(
      sessionDetail,
      richData ?? undefined,
      sessionInfo,
      richMessagesWithHookEvents,
    )
  }, [sessionDetail, richData, sessionInfo, richMessagesWithHookEvents])

  // Derived UI state from new hook
  const inputBarState = deriveInputBarState(sessionState, isLive)
  const connectionHealth = deriveConnectionHealth(sessionState)

  // ----- Early returns -----

  if (isLoading) {
    return (
      <div className="h-full flex flex-col overflow-hidden bg-gray-50 dark:bg-gray-950">
        <div className="flex items-center justify-between px-6 py-3 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
          <div className="h-5 w-32 bg-gray-200 rounded animate-pulse" />
          <div className="flex items-center gap-2">
            <div className="h-8 w-16 bg-gray-200 rounded animate-pulse" />
            <div className="h-8 w-16 bg-gray-200 rounded animate-pulse" />
          </div>
        </div>
        <div className="flex-1 p-6">
          <div className="max-w-4xl mx-auto">
            <Skeleton label="conversation" rows={4} withHeader={false} />
          </div>
        </div>
      </div>
    )
  }

  if (detailError) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50 dark:bg-gray-950">
        <ErrorState message={detailError.message} onBack={() => navigate(backUrl)} />
      </div>
    )
  }

  if (!blocks.length && !isLive && !sessionDetail) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50 dark:bg-gray-950">
        <EmptyState
          icon={<MessageSquare className="w-6 h-6 text-gray-400" />}
          title="No conversation data found"
          description="This session may have been deleted or moved."
          action={{ label: 'Back to sessions', onClick: () => navigate(backUrl) }}
        />
      </div>
    )
  }

  if (isFileGone && sessionDetail) {
    return (
      <div className="h-full flex flex-col overflow-hidden bg-gray-50 dark:bg-gray-950">
        <div className="flex items-center justify-between px-6 py-3 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
          <div className="flex items-center gap-4">
            <Link
              to={backUrl}
              className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 border border-gray-200 dark:border-gray-700 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              Sessions
            </Link>
            <span className="text-gray-300 dark:text-gray-600">|</span>
            <span className="font-medium text-gray-900 dark:text-gray-100">{projectName}</span>
          </div>
          <button
            type="button"
            onClick={handleResume}
            aria-label="Copy resume command to clipboard"
            title={`Session file missing from disk.\nProject: ${sessionDetail?.projectPath ?? 'unknown'}\nResume may fail if the directory no longer exists.`}
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-amber-500 dark:border-amber-400 text-amber-700 dark:text-amber-300 bg-white dark:bg-gray-800 rounded-md transition-colors hover:bg-amber-50 dark:hover:bg-amber-900/30 focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-offset-1"
          >
            <Terminal className="w-4 h-4" />
            <span>Resume</span>
          </button>
        </div>
        <div className="flex-1 flex overflow-hidden">
          <div className="flex-1 min-w-0 flex items-center justify-center">
            <div className="text-center max-w-md px-6">
              <FileX className="w-12 h-12 text-amber-400 mx-auto mb-4" />
              <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
                Session conversation data is no longer available
              </h2>
              <p className="text-sm text-gray-500 dark:text-gray-400 mb-6">
                The JSONL file for this session has been removed from disk. Session metrics are
                still available in the sidebar.
              </p>
              <Link
                to={backUrl}
                className="inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2"
              >
                Back to sessions
              </Link>
            </div>
          </div>
          <aside className="w-[300px] flex-shrink-0 border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-y-auto p-4 space-y-4 hidden lg:block">
            {sessionDetail.userPromptCount > 0 && (
              <SessionMetricsBar
                prompts={sessionDetail.userPromptCount}
                tokens={
                  sessionDetail.totalInputTokens != null && sessionDetail.totalOutputTokens != null
                    ? BigInt(sessionDetail.totalInputTokens) +
                      BigInt(sessionDetail.totalOutputTokens)
                    : null
                }
                filesRead={sessionDetail.filesReadCount}
                filesEdited={sessionDetail.filesEditedCount}
                reeditRate={
                  sessionDetail.filesEditedCount > 0
                    ? sessionDetail.reeditedFilesCount / sessionDetail.filesEditedCount
                    : null
                }
                commits={sessionDetail.commitCount}
                variant="vertical"
              />
            )}
            <FilesTouchedPanel
              files={buildFilesTouched(
                sessionDetail.filesRead ?? [],
                sessionDetail.filesEdited ?? [],
              )}
            />
            <CommitsPanel commits={sessionDetail.commits ?? []} />
          </aside>
        </div>
      </div>
    )
  }

  // Registry: chat bubbles (compact) vs developer terminal view
  const registry = verboseMode ? developerRegistry : chatRegistry

  return (
    <div className="h-full flex flex-col overflow-hidden bg-gray-50 dark:bg-gray-950">
      {/* Conversation Header */}
      <div className="flex items-center justify-between px-6 py-3 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-4">
          <Link
            to={backUrl}
            className="inline-flex items-center gap-1.5 px-2.5 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 border border-gray-200 dark:border-gray-700 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            Sessions
          </Link>
          <span className="text-gray-300 dark:text-gray-600">|</span>
          <span className="font-medium text-gray-900 dark:text-gray-100">{projectName}</span>
        </div>

        <div className="flex items-center gap-2">
          <ViewModeControls />
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={() => setPanelOpen(!panelOpen)}
            aria-pressed={panelOpen}
            className={cn(
              'p-1.5 rounded-md transition-colors',
              panelOpen
                ? 'bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400'
                : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800',
            )}
            title="Toggle detail panel"
          >
            <PanelRight className="w-4 h-4" />
          </button>
          <ShareModal
            sessionId={sessionId!}
            messages={session?.messages}
            projectName={projectName}
          />
          {/* Continue / Resume dropdown */}
          <div className="relative" ref={resumeMenuRef}>
            <button
              type="button"
              onClick={() => setResumeMenuOpen(!resumeMenuOpen)}
              disabled={!exportsReady}
              aria-label="Continue options"
              aria-expanded={resumeMenuOpen}
              aria-haspopup="menu"
              className={cn(
                'flex items-center gap-1.5 px-3 py-1.5 text-sm border rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                exportsReady
                  ? 'border-blue-500 dark:border-blue-400 text-blue-700 dark:text-blue-300 bg-white dark:bg-gray-800 hover:bg-blue-50 dark:hover:bg-blue-900/30 cursor-pointer'
                  : 'opacity-50 cursor-not-allowed border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-400',
              )}
            >
              <Terminal className="w-4 h-4" />
              <span>Continue</span>
              <ChevronDown
                className={cn('w-3.5 h-3.5 transition-transform', resumeMenuOpen && 'rotate-180')}
                aria-hidden="true"
              />
            </button>
            {resumeMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-56 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg z-50 py-1">
                <button
                  type="button"
                  onClick={() => {
                    handleCopyMarkdown()
                    setResumeMenuOpen(false)
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Copy className="w-4 h-4" />
                  Copy Full Transcript
                </button>
                <button
                  type="button"
                  onClick={() => {
                    handleResume()
                    setResumeMenuOpen(false)
                  }}
                  title={`claude --resume ${sessionId}\nProject: ${sessionDetail?.projectPath ?? 'unknown'}`}
                  className="w-full flex items-start gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Terminal className="w-4 h-4 mt-0.5 shrink-0" />
                  <span className="flex flex-col items-start">
                    <span>Resume Command</span>
                    <span className="text-[11px] text-gray-400 dark:text-gray-500 max-w-[180px] truncate">
                      {sessionDetail?.projectPath ?? ''}
                    </span>
                  </span>
                </button>
              </div>
            )}
          </div>
          {/* Export dropdown */}
          <div className="relative" ref={exportMenuRef}>
            <button
              type="button"
              onClick={() => setExportMenuOpen(!exportMenuOpen)}
              disabled={!exportsReady}
              aria-label="Export options"
              aria-expanded={exportMenuOpen}
              aria-haspopup="menu"
              className={cn(
                'flex items-center gap-1.5 px-2.5 py-1.5 text-sm border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 dark:text-gray-300 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1',
                exportsReady
                  ? 'hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer'
                  : 'opacity-50 cursor-not-allowed',
              )}
            >
              <Download className="w-4 h-4" />
              <span>Export</span>
              <ChevronDown
                className={cn('w-3.5 h-3.5 transition-transform', exportMenuOpen && 'rotate-180')}
                aria-hidden="true"
              />
            </button>
            {exportMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg z-50 py-1">
                <button
                  type="button"
                  onClick={() => {
                    handleExportHtml()
                    setExportMenuOpen(false)
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  HTML
                </button>
                <button
                  type="button"
                  onClick={() => {
                    handleExportPdf()
                    setExportMenuOpen(false)
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  PDF
                </button>
                <button
                  type="button"
                  onClick={() => {
                    handleExportMarkdown()
                    setExportMenuOpen(false)
                  }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  Markdown
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Two-column: Conversation + Sidebar */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Conversation thread + input bar */}
        <div className="flex-1 min-w-0 flex flex-col relative">
          {/* Cmd+F find bar */}
          {findOpen && (
            <div className="sticky top-0 z-10 bg-white dark:bg-slate-900 border-b border-slate-200 dark:border-white/[0.06] px-4 py-2 flex-shrink-0">
              <SearchInput
                value={findQuery}
                onChange={setFindQuery}
                placeholder="Find in conversation..."
                autoFocus
                shortcutHint="Cmd+F"
                onClose={() => {
                  setFindOpen(false)
                  setFindQuery('')
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') {
                    setFindOpen(false)
                    setFindQuery('')
                  }
                }}
              />
            </div>
          )}

          <div className="flex-1 min-h-0 overflow-y-auto">
            {verboseMode ? (
              <HistoryRichPane
                messages={richMessagesWithHookEvents}
                categoryCounts={historyCategoryCounts}
              />
            ) : (
              <FindProvider value={findQuery}>
                <ThreadHighlightProvider>
                  <ExpandProvider>
                    <div className="max-w-4xl mx-auto px-6 py-4">
                      <ErrorBoundary>
                        <ConversationThread blocks={blocks} renderers={registry} />
                      </ErrorBoundary>
                    </div>
                  </ExpandProvider>
                </ThreadHighlightProvider>
              </FindProvider>
            )}
          </div>

          <ConnectionBanner health={connectionHealth} />
          <ChatInputBar
            onSend={actions.sendMessage}
            state={inputBarState}
            contextPercent={contextPercent}
            mode={chatMode}
            onModeChange={handleModeChange}
            model={chatModel}
            onModelChange={setChatModel}
          />
        </div>

        {/* Right: Detail panel */}
        {panelOpen && panelData && (
          <SessionDetailPanel panelData={panelData} onClose={() => setPanelOpen(false)} inline />
        )}
      </div>
    </div>
  )
}
