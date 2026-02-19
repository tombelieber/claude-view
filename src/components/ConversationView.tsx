import { useState, useMemo, useEffect, useCallback, useRef } from 'react'
import { ThreadHighlightProvider } from '../contexts/ThreadHighlightContext'
import { ArrowLeft, ChevronDown, Copy, Download, MessageSquare, Eye, Code, FileX, Terminal } from 'lucide-react'
import { useParams, useNavigate, useOutletContext, Link, useSearchParams } from 'react-router-dom'
import { Virtuoso } from 'react-virtuoso'
import { useSession, isNotFoundError } from '../hooks/use-session'
import { useSessionMessages } from '../hooks/use-session-messages'
import { useProjectSessions } from '../hooks/use-projects'
import { useSessionDetail } from '../hooks/use-session-detail'
import { MessageTyped } from './MessageTyped'
import { ErrorBoundary } from './ErrorBoundary'
import { SessionMetricsBar } from './SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from './FilesTouchedPanel'
import { CommitsPanel } from './CommitsPanel'
import { generateStandaloneHtml, downloadHtml, exportToPdf, type ExportMetadata } from '../lib/export-html'
import { generateMarkdown, downloadMarkdown, copyToClipboard } from '../lib/export-markdown'
import { showToast } from '../lib/toast'
import { ExpandProvider } from '../contexts/ExpandContext'
import { Skeleton, ErrorState, EmptyState } from './LoadingStates'
import { cn } from '../lib/utils'
import { buildThreadMap, getThreadChain } from '../lib/thread-map'
import type { Message } from '../types/generated'
import type { ProjectSummary } from '../hooks/use-projects'

/** Strings that Claude Code emits as placeholder content (no real text) */
const EMPTY_CONTENT = new Set(['(no content)', ''])

function filterMessages(messages: Message[], mode: 'compact' | 'full'): Message[] {
  if (mode === 'full') return messages
  return messages.filter(msg => {
    if (msg.role === 'user') return true
    if (msg.role === 'assistant') {
      // Hide assistant messages with no real content (only tool calls, no text)
      if (EMPTY_CONTENT.has(msg.content.trim()) && !msg.thinking) return false
      return true
    }
    if (msg.role === 'tool_use') return false
    if (msg.role === 'tool_result') return false
    if (msg.role === 'system') return false
    if (msg.role === 'progress') return false
    if (msg.role === 'summary') return false
    return false
  })
}

export function ConversationView() {
  const { sessionId } = useParams()
  const navigate = useNavigate()
  const { summaries } = useOutletContext<{ summaries: ProjectSummary[] }>()

  // Fetch session detail first (uses /api/sessions/:id, no projectDir needed)
  // to get the project directory for the legacy session/messages endpoints
  const { data: sessionDetail, error: detailError } = useSessionDetail(sessionId || null)
  const projectDir = sessionDetail?.project ?? ''
  const project = summaries.find(p => p.name === projectDir)
  const projectName = project?.displayName || projectDir

  const [viewMode, setViewMode] = useState<'compact' | 'full'>('compact')
  const [exportMenuOpen, setExportMenuOpen] = useState(false)
  const exportMenuRef = useRef<HTMLDivElement>(null)
  const [resumeMenuOpen, setResumeMenuOpen] = useState(false)
  const resumeMenuRef = useRef<HTMLDivElement>(null)
  const [searchParams] = useSearchParams()

  // Build a deterministic "back to sessions" URL, preserving project/branch filters
  const backUrl = useMemo(() => {
    const preserved = new URLSearchParams()
    const project = searchParams.get('project')
    const branch = searchParams.get('branch')
    if (project) preserved.set('project', project)
    if (branch) preserved.set('branch', branch)
    const qs = preserved.toString()
    return qs ? `/sessions?${qs}` : '/sessions'
  }, [searchParams])
  // useSession and useSessionMessages now use /api/sessions/:id/* (no projectDir needed)
  const { data: session, error: sessionError } = useSession(sessionId || null)
  const {
    data: pagesData,
    isLoading: isMessagesLoading,
    error: messagesError,
    fetchPreviousPage,
    hasPreviousPage,
    isFetchingPreviousPage,
  } = useSessionMessages(sessionId || null)

  // Detect when DB has the session but the JSONL file is gone from disk
  const isFileGone = !!sessionDetail
    && (isNotFoundError(messagesError) || isNotFoundError(sessionError))

  // Only gate initial render on paginated messages — the full session fetch
  // loads in the background for export use. This ensures faster time-to-first-content.
  const isLoading = isFileGone ? false : (isMessagesLoading || (!sessionDetail && !detailError))
  const error = isFileGone ? null : (detailError || messagesError)
  const exportsReady = !!session

  const { data: sessionsPage } = useProjectSessions(projectDir || undefined, { limit: 500 })
  const sessionInfo = sessionsPage?.sessions.find(s => s.id === sessionId)

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
    showToast(ok ? 'Markdown copied to clipboard' : 'Failed to copy — check browser permissions', ok ? 2000 : 3000)
  }, [session, projectName, sessionId])

  const handleResume = useCallback(async () => {
    const projectPath = sessionDetail?.projectPath
    if (projectPath) {
      try {
        const res = await fetch(`/api/check-path?path=${encodeURIComponent(projectPath)}`)
        const data = await res.json()
        if (!data.exists) {
          showToast('Project path no longer exists — worktree may have been removed', 4000)
          return
        }
      } catch {
        // If the check fails (e.g. endpoint doesn't exist yet), proceed anyway
      }
    }
    const cmd = `claude --resume ${sessionId}`
    const ok = await copyToClipboard(cmd)
    showToast(
      ok ? 'Resume command copied — paste in terminal' : 'Failed to copy — check browser permissions',
      3000
    )
  }, [sessionId, sessionDetail])

  // Keyboard shortcuts: Cmd+Shift+E for HTML, Cmd+Shift+P for PDF
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Check for Cmd (Mac) or Ctrl (Windows/Linux)
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

  const allMessages = useMemo(
    () => pagesData?.pages.flatMap(page => page.messages) ?? [],
    [pagesData]
  )
  const totalMessages = pagesData?.pages[0]?.total ?? 0

  const filteredMessages = useMemo(
    () => allMessages.length > 0 ? filterMessages(allMessages, viewMode) : [],
    [allMessages, viewMode]
  )
  const hiddenCount = allMessages.length - filteredMessages.length

  // NOTE: In compact mode, heavy filtering may cause rapid sequential page fetches
  // since filtered content may not fill the viewport. This is bounded by hasPreviousPage
  // and acceptable for the initial implementation. Task 5 (server-side caching)
  // mitigates the server cost.
  const handleStartReached = useCallback(() => {
    if (hasPreviousPage && !isFetchingPreviousPage) {
      fetchPreviousPage()
    }
  }, [hasPreviousPage, isFetchingPreviousPage, fetchPreviousPage])

  const threadMap = useMemo(
    () => buildThreadMap(filteredMessages),
    [filteredMessages]
  )

  const getThreadChainForUuid = useCallback(
    (uuid: string) => getThreadChain(uuid, filteredMessages),
    [filteredMessages]
  )

  if (isLoading) {
    return (
      <div className="h-full flex flex-col overflow-hidden bg-gray-50 dark:bg-gray-950">
        {/* Header skeleton */}
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

  if (error) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50 dark:bg-gray-950">
        <ErrorState
          message={error.message}
          onBack={() => navigate(backUrl)}
        />
      </div>
    )
  }

  if (!session && !pagesData && !isFileGone) {
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
        {/* Header */}
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
            onClick={handleResume}
            aria-label="Copy resume command to clipboard"
            title={`Session file missing from disk.\nProject: ${sessionDetail?.projectPath ?? 'unknown'}\nResume may fail if the directory no longer exists.`}
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-amber-500 dark:border-amber-400 text-amber-700 dark:text-amber-300 bg-white dark:bg-gray-800 rounded-md transition-colors hover:bg-amber-50 dark:hover:bg-amber-900/30 focus-visible:ring-2 focus-visible:ring-amber-400 focus-visible:ring-offset-1"
          >
            <Terminal className="w-4 h-4" />
            <span>Resume</span>
          </button>
        </div>

        {/* Two-column: Message + Sidebar */}
        <div className="flex-1 flex overflow-hidden">
          {/* Left: File-gone notice */}
          <div className="flex-1 min-w-0 flex items-center justify-center">
            <div className="text-center max-w-md px-6">
              <FileX className="w-12 h-12 text-amber-400 mx-auto mb-4" />
              <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100 mb-2">
                Session conversation data is no longer available
              </h2>
              <p className="text-sm text-gray-500 dark:text-gray-400 mb-6">
                The JSONL file for this session has been removed from disk.
                Session metrics are still available in the sidebar.
              </p>
              <Link
                to={backUrl}
                className="inline-flex items-center gap-1.5 px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-md transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2"
              >
                Back to sessions
              </Link>
            </div>
          </div>

          {/* Right: Metrics sidebar — still renders from DB data */}
          <aside className="w-[300px] flex-shrink-0 border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-y-auto p-4 space-y-4 hidden lg:block">
            {sessionDetail.userPromptCount > 0 && (
              <SessionMetricsBar
                prompts={sessionDetail.userPromptCount}
                tokens={
                  sessionDetail.totalInputTokens != null && sessionDetail.totalOutputTokens != null
                    ? BigInt(sessionDetail.totalInputTokens) + BigInt(sessionDetail.totalOutputTokens)
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
                sessionDetail.filesEdited ?? []
              )}
            />
            <CommitsPanel commits={sessionDetail.commits ?? []} />
          </aside>
        </div>
      </div>
    )
  }

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
          {/* View mode toggle */}
          <div className="flex items-center gap-1 bg-gray-100 dark:bg-gray-800 rounded-md p-0.5">
            <button
              onClick={() => setViewMode('compact')}
              aria-pressed={viewMode === 'compact'}
              className={cn(
                'px-3 py-1.5 text-xs font-medium rounded transition-colors duration-200',
                viewMode === 'compact'
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
              )}
            >
              <Eye className="w-3.5 h-3.5 inline mr-1.5" aria-hidden="true" />
              Compact
            </button>
            <button
              onClick={() => setViewMode('full')}
              aria-pressed={viewMode === 'full'}
              className={cn(
                'px-3 py-1.5 text-xs font-medium rounded transition-colors duration-200',
                viewMode === 'full'
                  ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300 cursor-pointer'
              )}
            >
              <Code className="w-3.5 h-3.5 inline mr-1.5" aria-hidden="true" />
              Verbose
            </button>
          </div>
          {viewMode === 'compact' && hiddenCount > 0 && (
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {hiddenCount} hidden
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          {/* Continue / Resume dropdown */}
          <div className="relative" ref={resumeMenuRef}>
            <button
              onClick={() => setResumeMenuOpen(!resumeMenuOpen)}
              disabled={!exportsReady}
              aria-label="Continue options"
              aria-expanded={resumeMenuOpen}
              aria-haspopup="menu"
              className={cn(
                "flex items-center gap-1.5 px-3 py-1.5 text-sm border rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1",
                exportsReady
                  ? "border-blue-500 dark:border-blue-400 text-blue-700 dark:text-blue-300 bg-white dark:bg-gray-800 hover:bg-blue-50 dark:hover:bg-blue-900/30 cursor-pointer"
                  : "opacity-50 cursor-not-allowed border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-400"
              )}
            >
              <Terminal className="w-4 h-4" />
              <span>Continue</span>
              <ChevronDown className={cn("w-3.5 h-3.5 transition-transform", resumeMenuOpen && "rotate-180")} aria-hidden="true" />
            </button>

            {resumeMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-56 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg z-50 py-1">
                <button
                  onClick={() => { handleCopyMarkdown(); setResumeMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Copy className="w-4 h-4" />
                  Copy Full Transcript
                </button>
                <button
                  onClick={() => { handleResume(); setResumeMenuOpen(false) }}
                  title={`claude --resume ${sessionId}\nProject: ${sessionDetail?.projectPath ?? 'unknown'}`}
                  className="w-full flex items-start gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Terminal className="w-4 h-4 mt-0.5 shrink-0" />
                  <span className="flex flex-col items-start">
                    <span>Resume Command</span>
                    <span className="text-[11px] text-gray-400 dark:text-gray-500 max-w-[180px] truncate">{sessionDetail?.projectPath ?? ''}</span>
                  </span>
                </button>
              </div>
            )}
          </div>

          {/* Export overflow menu */}
          <div className="relative" ref={exportMenuRef}>
            <button
              onClick={() => setExportMenuOpen(!exportMenuOpen)}
              disabled={!exportsReady}
              aria-label="Export options"
              aria-expanded={exportMenuOpen}
              aria-haspopup="menu"
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1.5 text-sm border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 dark:text-gray-300 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1",
                exportsReady
                  ? "hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                  : "opacity-50 cursor-not-allowed"
              )}
            >
              <Download className="w-4 h-4" />
              <span>Export</span>
              <ChevronDown className={cn("w-3.5 h-3.5 transition-transform", exportMenuOpen && "rotate-180")} aria-hidden="true" />
            </button>

            {exportMenuOpen && (
              <div className="absolute right-0 top-full mt-1 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-md shadow-lg z-50 py-1">
                <button
                  onClick={() => { handleExportHtml(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  HTML
                </button>
                <button
                  onClick={() => { handleExportPdf(); setExportMenuOpen(false) }}
                  className="w-full flex items-center gap-2 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 cursor-pointer"
                >
                  <Download className="w-4 h-4" />
                  PDF
                </button>
                <button
                  onClick={() => { handleExportMarkdown(); setExportMenuOpen(false) }}
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
        {/* Left: Conversation messages */}
        <div className="flex-1 min-w-0">
          <ThreadHighlightProvider>
          <ExpandProvider>
            <Virtuoso
              data={filteredMessages}
              startReached={handleStartReached}
              initialTopMostItemIndex={Math.max(0, filteredMessages.length - 1)}
              followOutput="smooth"
              itemContent={(index, message) => {
                const thread = message.uuid ? threadMap.get(message.uuid) : undefined
                return (
                  <div className="max-w-4xl mx-auto px-6 pb-4">
                    <ErrorBoundary key={message.uuid || index}>
                      <MessageTyped
                        message={message}
                        messageIndex={index}
                        messageType={message.role}
                        metadata={message.metadata}
                        parentUuid={thread?.parentUuid}
                        indent={thread?.indent ?? 0}
                        isChildMessage={thread?.isChild ?? false}
                        onGetThreadChain={getThreadChainForUuid}
                      />
                    </ErrorBoundary>
                  </div>
                )
              }}
              components={{
                Header: () => (
                  isFetchingPreviousPage ? (
                    <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
                      Loading older messages...
                    </div>
                  ) : hasPreviousPage ? (
                    <div className="h-6" />
                  ) : filteredMessages.length > 0 ? (
                    <div className="max-w-4xl mx-auto px-6 py-4 text-center text-sm text-gray-400 dark:text-gray-500">
                      Beginning of conversation
                    </div>
                  ) : (
                    <div className="h-6" />
                  )
                ),
                Footer: () => (
                  filteredMessages.length > 0 ? (
                    <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400 dark:text-gray-500">
                      {totalMessages} messages
                      {viewMode === 'compact' && hiddenCount > 0 && (
                        <> &bull; {hiddenCount} hidden in compact view</>
                      )}
                      {sessionInfo && sessionInfo.toolCallCount > 0 && (
                        <> &bull; {sessionInfo.toolCallCount} tool calls</>
                      )}
                    </div>
                  ) : null
                )
              }}
              increaseViewportBy={{ top: 400, bottom: 400 }}
              className="h-full overflow-auto"
            />
          </ExpandProvider>
          </ThreadHighlightProvider>
        </div>

        {/* Right: Metrics Sidebar */}
        <aside className="w-[300px] flex-shrink-0 border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-y-auto p-4 space-y-4 hidden lg:block">
          {/* Metrics (vertical layout per plan B9.3) */}
          {sessionInfo && sessionInfo.userPromptCount > 0 && (
            <SessionMetricsBar
              prompts={sessionInfo.userPromptCount}
              tokens={
                sessionInfo.totalInputTokens != null && sessionInfo.totalOutputTokens != null
                  ? BigInt(sessionInfo.totalInputTokens) + BigInt(sessionInfo.totalOutputTokens)
                  : null
              }
              filesRead={sessionInfo.filesReadCount}
              filesEdited={sessionInfo.filesEditedCount}
              reeditRate={
                sessionInfo.filesEditedCount > 0
                  ? sessionInfo.reeditedFilesCount / sessionInfo.filesEditedCount
                  : null
              }
              commits={sessionInfo.commitCount}
              variant="vertical"
            />
          )}

          {/* Files Touched */}
          {sessionDetail && (
            <FilesTouchedPanel
              files={buildFilesTouched(
                sessionDetail.filesRead ?? [],
                sessionDetail.filesEdited ?? []
              )}
            />
          )}

          {/* Linked Commits */}
          {sessionDetail && (
            <CommitsPanel commits={sessionDetail.commits ?? []} />
          )}
        </aside>
      </div>
    </div>
  )
}
