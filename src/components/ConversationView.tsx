import { useState, useMemo, useEffect, useCallback } from 'react'
import { ArrowLeft, Copy, Download, MessageSquare, Eye, Code } from 'lucide-react'
import { useParams, useNavigate, useOutletContext } from 'react-router-dom'
import { Virtuoso } from 'react-virtuoso'
import { useSession } from '../hooks/use-session'
import { useProjectSessions } from '../hooks/use-projects'
import { useSessionDetail } from '../hooks/use-session-detail'
import { MessageTyped } from './MessageTyped'
import { ErrorBoundary } from './ErrorBoundary'
import { SessionMetricsBar } from './SessionMetricsBar'
import { FilesTouchedPanel, buildFilesTouched } from './FilesTouchedPanel'
import { CommitsPanel } from './CommitsPanel'
import { generateStandaloneHtml, downloadHtml, exportToPdf } from '../lib/export-html'
import { generateMarkdown, downloadMarkdown, copyToClipboard } from '../lib/export-markdown'
import { showToast } from '../lib/toast'
import { ExpandProvider } from '../contexts/ExpandContext'
import { sessionIdFromSlug } from '../lib/url-slugs'
import { Skeleton, ErrorState, EmptyState } from './LoadingStates'
import { cn } from '../lib/utils'
import type { Message } from '../types/generated'
import type { ProjectSummary } from '../hooks/use-projects'

function filterMessages(messages: Message[], mode: 'compact' | 'full'): Message[] {
  if (mode === 'full') return messages
  return messages.filter(msg => {
    if (msg.role === 'user') return true
    if (msg.role === 'assistant') return true
    if (msg.role === 'tool_use') return false
    if (msg.role === 'tool_result') return false
    if (msg.role === 'system') return false
    if (msg.role === 'progress') return false
    if (msg.role === 'summary') return false
    return false
  })
}

export function ConversationView() {
  const { projectId, slug } = useParams()
  const navigate = useNavigate()
  const { summaries } = useOutletContext<{ summaries: ProjectSummary[] }>()

  const projectDir = projectId ? decodeURIComponent(projectId) : ''
  const project = summaries.find(p => p.name === projectDir)
  const projectName = project?.displayName || projectDir
  const sessionId = slug ? sessionIdFromSlug(slug) : ''

  const [viewMode, setViewMode] = useState<'compact' | 'full'>('compact')

  const handleBack = () => navigate(-1)
  const { data: session, isLoading, error } = useSession(projectDir, sessionId)
  const { data: sessionsPage } = useProjectSessions(projectDir || undefined, { limit: 500 })
  const sessionInfo = sessionsPage?.sessions.find(s => s.id === sessionId)
  const { data: sessionDetail } = useSessionDetail(sessionId || null)

  const handleExportHtml = useCallback(() => {
    if (!session) return
    const html = generateStandaloneHtml(session.messages)
    const filename = `conversation-${sessionId}.html`
    downloadHtml(html, filename)
  }, [session, sessionId])

  const handleExportPdf = useCallback(() => {
    if (!session) return
    exportToPdf(session.messages)
  }, [session])

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
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [handleExportHtml, handleExportPdf])

  const filteredMessages = useMemo(
    () => session ? filterMessages(session.messages, viewMode) : [],
    [session?.messages, viewMode]
  )
  const hiddenCount = session ? session.messages.length - filteredMessages.length : 0

  if (isLoading) {
    return (
      <div className="h-full flex flex-col overflow-hidden bg-gray-50">
        {/* Header skeleton */}
        <div className="flex items-center justify-between px-6 py-3 bg-white border-b border-gray-200">
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
      <div className="h-full flex items-center justify-center bg-gray-50">
        <ErrorState
          message={error.message}
          onBack={handleBack}
        />
      </div>
    )
  }

  if (!session) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50">
        <EmptyState
          icon={<MessageSquare className="w-6 h-6 text-gray-400" />}
          title="No conversation data found"
          description="This session may have been deleted or moved."
          action={{ label: 'Go back', onClick: handleBack }}
        />
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col overflow-hidden bg-gray-50">
      {/* Conversation Header */}
      <div className="flex items-center justify-between px-6 py-3 bg-white border-b border-gray-200">
        <div className="flex items-center gap-4">
          <button
            onClick={handleBack}
            aria-label="Go back"
            className="flex items-center gap-1 text-gray-600 hover:text-gray-900 transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
          >
            <ArrowLeft className="w-4 h-4" />
            <span className="text-sm">Back to sessions</span>
          </button>
          <span className="text-gray-300">|</span>
          <span className="font-medium text-gray-900">{projectName}</span>
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
              Smart
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
              Full
            </button>
          </div>
          {viewMode === 'compact' && hiddenCount > 0 && (
            <span className="text-xs text-gray-400 dark:text-gray-500">
              {hiddenCount} hidden
            </span>
          )}
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={handleExportHtml}
            aria-label="Export as HTML"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>HTML</span>
            <Download className="w-4 h-4" />
          </button>
          <button
            onClick={handleExportPdf}
            aria-label="Export as PDF"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>PDF</span>
            <Download className="w-4 h-4" />
          </button>
          <button
            onClick={handleExportMarkdown}
            aria-label="Export as Markdown"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>MD</span>
            <Download className="w-4 h-4" />
          </button>
          <button
            onClick={handleCopyMarkdown}
            aria-label="Copy conversation as Markdown"
            className="flex items-center gap-2 px-3 py-1.5 text-sm border border-gray-300 bg-white hover:bg-gray-50 rounded-md transition-colors focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          >
            <span>Copy</span>
            <Copy className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Two-column: Conversation + Sidebar */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left: Conversation messages */}
        <div className="flex-1 min-w-0">
          <ExpandProvider>
            <Virtuoso
              data={filteredMessages}
              itemContent={(index, message) => (
                <div className="max-w-4xl mx-auto px-6 pb-4">
                  <ErrorBoundary key={message.uuid || index}>
                    <MessageTyped
                      message={message}
                      messageIndex={index}
                      messageType={message.role}
                      metadata={message.metadata}
                      parentUuid={message.parent_uuid ?? undefined}
                    />
                  </ErrorBoundary>
                </div>
              )}
              components={{
                Header: () => <div className="h-6" />,
                Footer: () => (
                  filteredMessages.length > 0 ? (
                    <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400">
                      {session.metadata.totalMessages} messages
                      {viewMode === 'compact' && hiddenCount > 0 && (
                        <> &bull; {hiddenCount} hidden in compact view</>
                      )}
                      {session.metadata.toolCallCount > 0 && (
                        <> &bull; {session.metadata.toolCallCount} tool calls</>
                      )}
                    </div>
                  ) : null
                )
              }}
              increaseViewportBy={{ top: 400, bottom: 400 }}
              initialTopMostItemIndex={0}
              className="h-full overflow-auto"
            />
          </ExpandProvider>
        </div>

        {/* Right: Metrics Sidebar */}
        <aside className="w-[300px] flex-shrink-0 border-l border-gray-200 bg-white overflow-y-auto p-4 space-y-4 hidden lg:block">
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
