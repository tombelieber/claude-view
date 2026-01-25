import { ArrowLeft, Download, Loader2 } from 'lucide-react'
import { useSession } from '../hooks/use-session'
import { Message } from './Message'
import { generateStandaloneHtml, downloadHtml } from '../lib/export-html'

interface ConversationViewProps {
  projectDir: string
  projectName: string
  sessionId: string
  onBack: () => void
}

export function ConversationView({
  projectDir,
  projectName,
  sessionId,
  onBack,
}: ConversationViewProps) {
  const { data: session, isLoading, error } = useSession(projectDir, sessionId)

  const handleExport = () => {
    if (!session) return
    const html = generateStandaloneHtml(session.messages)
    const filename = `claude-conversation-${sessionId}.html`
    downloadHtml(html, filename)
  }

  if (isLoading) {
    return (
      <main className="flex-1 flex items-center justify-center bg-gray-50">
        <div className="flex items-center gap-3 text-gray-600">
          <Loader2 className="w-5 h-5 animate-spin" />
          <span>Loading conversation...</span>
        </div>
      </main>
    )
  }

  if (error) {
    return (
      <main className="flex-1 flex items-center justify-center bg-gray-50">
        <div className="text-center text-red-600">
          <p className="font-medium">Failed to load conversation</p>
          <p className="text-sm mt-1">{error.message}</p>
          <button
            onClick={onBack}
            className="mt-4 px-4 py-2 text-sm text-gray-600 hover:text-gray-900"
          >
            Go back
          </button>
        </div>
      </main>
    )
  }

  if (!session) {
    return (
      <main className="flex-1 flex items-center justify-center bg-gray-50">
        <div className="text-center text-gray-500">
          <p>No conversation data found</p>
          <button
            onClick={onBack}
            className="mt-4 px-4 py-2 text-sm text-gray-600 hover:text-gray-900"
          >
            Go back
          </button>
        </div>
      </main>
    )
  }

  return (
    <main className="flex-1 flex flex-col overflow-hidden bg-gray-50">
      {/* Conversation Header */}
      <div className="flex items-center justify-between px-6 py-3 bg-white border-b border-gray-200">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="flex items-center gap-1 text-gray-600 hover:text-gray-900 transition-colors"
          >
            <ArrowLeft className="w-4 h-4" />
            <span className="text-sm">Back to sessions</span>
          </button>
          <span className="text-gray-300">|</span>
          <span className="font-medium text-gray-900">{projectName}</span>
        </div>

        <button
          onClick={handleExport}
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-white bg-blue-500 hover:bg-blue-600 rounded-md transition-colors"
        >
          <span>Export HTML</span>
          <Download className="w-4 h-4" />
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto px-6 py-6">
          <div className="space-y-4">
            {session.messages.map((message, index) => (
              <Message key={index} message={message} />
            ))}
          </div>

          {session.messages.length > 0 && (
            <div className="mt-6 text-center text-sm text-gray-400">
              {session.metadata.totalMessages} messages
              {session.metadata.toolCallCount > 0 && (
                <> &bull; {session.metadata.toolCallCount} tool calls</>
              )}
            </div>
          )}
        </div>
      </div>
    </main>
  )
}
