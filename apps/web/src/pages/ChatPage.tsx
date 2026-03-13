import { useCallback, useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { ChatInputBar } from '../components/chat/ChatInputBar'
import { ModelSelector } from '../components/chat/ModelSelector'
import { ConversationThread } from '../components/conversation/ConversationThread'
import { chatRegistry } from '../components/conversation/blocks/chat/registry'
import { developerRegistry } from '../components/conversation/blocks/developer/registry'
import { SessionSidebar } from '../components/conversation/sidebar/SessionSidebar'
import { useConversation } from '../hooks/use-conversation'
import { deriveInputBarState } from '../lib/control-status-map'
import { getContextLimit } from '../lib/model-context-windows'
import type { PermissionMode } from '../types/control'

const DEFAULT_MODEL = 'claude-sonnet-4-20250514'
const MODEL_STORAGE_KEY = 'claude-view:last-model'

// NOTE: Display mode (chat/developer) is NOT the same as permission mode (default/plan/auto/etc.)
// Display mode: which block renderers to use — client-side only, always toggleable
// Permission mode: SDK permissionMode — sent via setMode to sidecar
type DisplayMode = 'chat' | 'developer'

function ModeToggle({ mode, onChange }: { mode: DisplayMode; onChange: (m: DisplayMode) => void }) {
  return (
    <div className="flex items-center gap-1 p-0.5 rounded-md bg-gray-100 dark:bg-gray-800 text-sm">
      {(['chat', 'developer'] as const).map((m) => (
        <button
          type="button"
          key={m}
          onClick={() => onChange(m)}
          className={[
            'px-2.5 py-1 rounded transition-colors capitalize',
            mode === m
              ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
              : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
          ].join(' ')}
        >
          {m}
        </button>
      ))}
    </div>
  )
}

export function ChatPage() {
  const navigate = useNavigate()
  const { sessionId } = useParams<{ sessionId?: string }>()

  const { blocks, actions, sessionInfo } = useConversation(sessionId)

  // Model selection persisted in localStorage (used at session creation)
  const [selectedModel, setSelectedModel] = useState<string>(() => {
    try {
      return localStorage.getItem(MODEL_STORAGE_KEY) ?? DEFAULT_MODEL
    } catch {
      return DEFAULT_MODEL
    }
  })

  const handleModelChange = useCallback((model: string) => {
    setSelectedModel(model)
    try {
      localStorage.setItem(MODEL_STORAGE_KEY, model)
    } catch {
      /* noop */
    }
  }, [])

  // Display mode persisted in localStorage
  const [displayMode, setDisplayMode] = useState<DisplayMode>(() => {
    try {
      return (localStorage.getItem('chat-display-mode') as DisplayMode) ?? 'chat'
    } catch {
      return 'chat'
    }
  })

  const handleModeChange = useCallback((m: DisplayMode) => {
    setDisplayMode(m)
    try {
      localStorage.setItem('chat-display-mode', m)
    } catch {
      /* noop */
    }
  }, [])

  const registry = displayMode === 'chat' ? chatRegistry : developerRegistry
  const inputBarState = deriveInputBarState(
    sessionInfo.sessionState,
    sessionInfo.isLive,
    sessionInfo.canResumeLazy,
  )

  // Context gauge from live WS token data
  const contextWindow = getContextLimit(
    null,
    sessionInfo.totalInputTokens || undefined,
    sessionInfo.contextWindowSize || null,
  )
  const contextPercent = sessionInfo.totalInputTokens
    ? Math.round((sessionInfo.totalInputTokens / contextWindow) * 100)
    : undefined

  const handleSend = useCallback(
    (text: string) => {
      if (!sessionId) {
        // No session yet — create one first, then navigate
        fetch('/api/control/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ model: selectedModel, initialMessage: text }),
        })
          .then((r) => r.json())
          .then((data) => {
            if (data.sessionId) navigate(`/chat/${data.sessionId}`)
          })
          .catch(() => {
            /* silently fail */
          })
        return
      }
      actions.sendMessage(text)
    },
    [sessionId, actions, navigate, selectedModel],
  )

  const handleModeChangePermission = useCallback(
    (mode: PermissionMode) => {
      actions.setPermissionMode(mode)
    },
    [actions],
  )

  return (
    <div className="flex h-full overflow-hidden">
      {/* Sidebar */}
      <SessionSidebar />

      {/* Main area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
          <div className="flex items-center gap-3">
            {sessionInfo.isLive && (
              <span className="flex items-center gap-1.5 text-xs text-green-500">
                <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                Live
              </span>
            )}
          </div>
          <ModeToggle mode={displayMode} onChange={handleModeChange} />
        </div>

        {/* Thread */}
        <div className="flex-1 overflow-y-auto">
          {blocks.length === 0 ? (
            <div className="flex items-center justify-center h-full text-gray-400 dark:text-gray-500">
              <div className="text-center">
                <p className="text-lg font-medium mb-2">Start a conversation</p>
                <p className="text-sm mb-4">Send a message to begin.</p>
                {!sessionId && (
                  <div className="flex justify-center">
                    <ModelSelector model={selectedModel} onModelChange={handleModelChange} />
                  </div>
                )}
              </div>
            </div>
          ) : (
            <div className="max-w-3xl mx-auto px-4 py-6">
              <ConversationThread blocks={blocks} renderers={registry} />
            </div>
          )}
        </div>

        {/* Input */}
        <div className="flex-shrink-0 border-t border-gray-200 dark:border-gray-800">
          <div className="max-w-3xl mx-auto px-4 py-3">
            <ChatInputBar
              onSend={handleSend}
              state={inputBarState}
              onModeChange={handleModeChangePermission}
              contextPercent={contextPercent}
            />
          </div>
        </div>
      </div>
    </div>
  )
}
