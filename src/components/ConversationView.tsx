import { useEffect, useCallback } from 'react'
import { ArrowLeft, Download, Loader2 } from 'lucide-react'
import { useParams, useNavigate, useOutletContext } from 'react-router-dom'
import { Virtuoso } from 'react-virtuoso'
import { useSession } from '../hooks/use-session'
import { Message } from './Message'
import { generateStandaloneHtml, downloadHtml, exportToPdf } from '../lib/export-html'
import { ExpandProvider } from '../contexts/ExpandContext'
import { sessionIdFromSlug } from '../lib/url-slugs'
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
      <div className="h-full flex items-center justify-center bg-gray-50">
        <div className="flex items-center gap-3 text-gray-600">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Loading conversation...</span>
        </div>
      </div>
    )
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50">
        <div className="text-center text-red-600">
          <p className="font-medium">Failed to load conversation</p>
          <p className="text-sm mt-1">{error.message}</p>
          <button
            onClick={handleBack}
            className="mt-4 px-4 py-2 text-sm text-gray-600 hover:text-gray-900"
          >
            Go back
          </button>
        </div>
      </div>
    )
  }

  if (!session) {
    return (
      <div className="h-full flex items-center justify-center bg-gray-50">
        <div className="text-center text-gray-500">
          <p>No conversation data found</p>
          <button
            onClick={handleBack}
            className="mt-4 px-4 py-2 text-sm text-gray-600 hover:text-gray-900"
          >
            Go back
          </button>
        </div>
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
