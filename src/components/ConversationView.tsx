import { useEffect, useCallback } from 'react'
import { ArrowLeft, Download, MessageSquare } from 'lucide-react'
import { useParams, useNavigate, useOutletContext } from 'react-router-dom'
import { Virtuoso } from 'react-virtuoso'
import { useSession } from '../hooks/use-session'
import { useProjectSessions } from '../hooks/use-projects'
import { Message } from './Message'
import { SessionMetricsBar } from './SessionMetricsBar'
import { generateStandaloneHtml, downloadHtml, exportToPdf } from '../lib/export-html'
import { ExpandProvider } from '../contexts/ExpandContext'
import { sessionIdFromSlug } from '../lib/url-slugs'
import { Skeleton, ErrorState, EmptyState } from './LoadingStates'
import type { ProjectSummary } from '../hooks/use-projects'

export function ConversationView() {
  const { projectId, slug } = useParams()
  const navigate = useNavigate()
  const { summaries } = useOutletContext<{ summaries: ProjectSummary[] }>()

  const projectDir = projectId ? decodeURIComponent(projectId) : ''
  const project = summaries.find(p => p.name === projectDir)
  const projectName = project?.displayName || projectDir
  const sessionId = slug ? sessionIdFromSlug(slug) : ''

  const handleBack = () => navigate(-1)
  const { data: session, isLoading, error } = useSession(projectDir, sessionId)
  const { data: sessionsPage } = useProjectSessions(projectDir || undefined, { limit: 500 })
  const sessionInfo = sessionsPage?.sessions.find(s => s.id === sessionId)

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
        </div>
      </div>

      {/* Session Metrics Bar */}
      {sessionInfo && sessionInfo.userPromptCount > 0 && (
        <div className="px-6 py-2 bg-white border-b border-gray-200">
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
          />
        </div>
      )}

      {/* Messages */}
      <ExpandProvider>
        <Virtuoso
          data={session.messages}
          itemContent={(index, message) => (
            <div className="max-w-4xl mx-auto px-6 pb-4">
              <Message key={message.id || index} message={message} messageIndex={index} />
            </div>
          )}
          components={{
            Header: () => <div className="h-6" />,
            Footer: () => (
              session.messages.length > 0 ? (
                <div className="max-w-4xl mx-auto px-6 py-6 text-center text-sm text-gray-400">
                  {session.metadata.totalMessages} messages
                  {session.metadata.toolCallCount > 0 && (
                    <> &bull; {session.metadata.toolCallCount} tool calls</>
                  )}
                </div>
              ) : null
            )
          }}
          increaseViewportBy={{ top: 400, bottom: 400 }}
          initialTopMostItemIndex={0}
          className="flex-1 overflow-auto"
        />
      </ExpandProvider>
    </div>
  )
}
