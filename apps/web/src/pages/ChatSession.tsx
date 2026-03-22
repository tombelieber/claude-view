import { useCallback, useEffect, useRef, useState } from 'react'
import { ChatInputBar } from '../components/chat/ChatInputBar'
import { McpPanel } from '../components/chat/McpPanel'
import { ModelSelector } from '../components/chat/ModelSelector'
import { TakeoverDialog } from '../components/chat/TakeoverDialog'
import { ConversationThread } from '../components/conversation/ConversationThread'
import { chatRegistry } from '../components/conversation/blocks/chat/registry'
import { developerRegistry } from '../components/conversation/blocks/developer/registry'

import { ExpandProvider } from '../contexts/ExpandContext'
import { ConversationActionsProvider } from '../contexts/conversation-actions-context'
import { useChatPanel } from '../hooks/use-chat-panel'
import { useCommandExecutor } from '../hooks/use-command-executor'
import { useContextPercent } from '../hooks/use-context-percent'
import type { LiveContextData } from '../hooks/use-context-percent'
import { resolveSessionModel, useModelOptions } from '../hooks/use-models'
import { useRichSessionData } from '../hooks/use-rich-session-data'

import { useSessionDetail } from '../hooks/use-session-detail'
import { useTelemetryPrompt } from '../hooks/use-telemetry-prompt'
import { useTrackEvent } from '../hooks/use-track-event'
import type { LiveStatus } from '../lib/live-status'
import type { PermissionMode } from '../types/control'

const DEFAULT_MODEL = 'claude-sonnet-4-20250514'
const MODEL_STORAGE_KEY = 'claude-view:last-model'
const MODE_STORAGE_KEY = 'claude-view:last-mode'

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

interface ChatSessionProps {
  sessionId: string | undefined
  liveStatus: LiveStatus
  /** Decoded project path from Live Monitor — passed to SDK on resume/fork for correct cwd. */
  liveProjectPath?: string
  /** Authoritative context gauge data from Live Monitor SSE. Undefined when no live session. */
  liveContextData?: LiveContextData
  /** Called when a new session is created from a blank panel (dockview transition). */
  onSessionCreated?: (sessionId: string) => void
}

export function ChatSession({
  sessionId,
  liveStatus,
  liveProjectPath,
  liveContextData,
  onSessionCreated,
}: ChatSessionProps) {
  const trackEvent = useTrackEvent()
  const { recordSessionView } = useTelemetryPrompt()

  useEffect(() => {
    if (sessionId) {
      trackEvent('session_opened')
      recordSessionView()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId])

  // FSM: single hook replaces useConversation + useSendHandler + useSessionCapabilities
  const { store, dispatch, pendingCmdsRef, blocks, inputBar, viewMode, connectionStatus } =
    useChatPanel(sessionId, liveProjectPath)
  const { channel } = useCommandExecutor(store, dispatch, pendingCmdsRef)

  // Dispatch LIVE_STATUS_CHANGED when liveStatus or projectPath changes
  useEffect(() => {
    dispatch({ type: 'LIVE_STATUS_CHANGED', status: liveStatus, projectPath: liveProjectPath })
  }, [liveStatus, liveProjectPath, dispatch])

  // Notify dockview when FSM creates a new session (blank panel → create flow)
  const notifiedSessionRef = useRef<string | null>(null)
  useEffect(() => {
    if (!onSessionCreated) return
    const panelSessionId = 'sessionId' in store.panel ? store.panel.sessionId : undefined
    if (
      panelSessionId &&
      panelSessionId !== sessionId &&
      panelSessionId !== notifiedSessionRef.current
    ) {
      notifiedSessionRef.current = panelSessionId
      onSessionCreated(panelSessionId)
    }
  }, [store.panel, sessionId, onSessionCreated])

  const { data: richData } = useRichSessionData(sessionId || null)
  const { data: sessionDetail } = useSessionDetail(sessionId || null)

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

  // Thinking budget (optimistic local state)
  const [thinkingBudget, setThinkingBudget] = useState<number | null>(null)
  const [mcpPanelOpen, setMcpPanelOpen] = useState(false)

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

  // Permission mode — persisted globally (like model), applied at session creation/resume
  const [permMode, setPermMode] = useState<PermissionMode>(() => {
    try {
      const stored = localStorage.getItem(MODE_STORAGE_KEY) as PermissionMode | null
      return stored ?? 'default'
    } catch {
      return 'default'
    }
  })

  const contextPercent = useContextPercent(liveContextData, richData?.contextWindowTokens)

  // Send via FSM dispatch — pass model + permissionMode so create/resume use the right values
  const handleSend = useCallback(
    (text: string) => {
      dispatch({
        type: 'SEND_MESSAGE',
        text,
        localId: crypto.randomUUID(),
        model: selectedModel,
        permissionMode: permMode,
      })
    },
    [dispatch, selectedModel, permMode],
  )

  const handleModeChangePermission = useCallback(
    (mode: PermissionMode) => {
      setPermMode(mode)
      try {
        localStorage.setItem(MODE_STORAGE_KEY, mode)
      } catch {
        /* noop */
      }
      dispatch({ type: 'SET_PERMISSION_MODE', mode })
    },
    [dispatch],
  )

  // --- Command palette ---
  const { options: modelOptions } = useModelOptions()

  // History session: auto-select the session's primary model if SDK-supported,
  // otherwise keep the user's default (from localStorage).
  useEffect(() => {
    if (!sessionId || !sessionDetail?.primaryModel || modelOptions.length === 0) return
    const resolved = resolveSessionModel(sessionDetail.primaryModel, modelOptions)
    if (resolved) setSelectedModel(resolved)
  }, [sessionId, sessionDetail?.primaryModel, modelOptions])

  // Takeover dialog state
  const [showTakeover, setShowTakeover] = useState(false)

  return (
    <div className="flex-1 flex flex-col overflow-hidden" data-panel-mode={viewMode}>
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
        <div className="flex items-center gap-3">
          {viewMode === 'watching' ? (
            <span className="flex items-center gap-1.5 text-xs text-blue-500">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse" />
              Watching
            </span>
          ) : viewMode === 'connecting' ? (
            <span className="flex items-center gap-1.5 text-xs text-amber-500">
              <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
              Connecting
            </span>
          ) : viewMode === 'active' ? (
            <span className="flex items-center gap-1.5 text-xs text-green-500">
              <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
              Live
            </span>
          ) : viewMode === 'error' ? (
            <span className="flex items-center gap-1.5 text-xs text-red-500">
              <span className="w-1.5 h-1.5 rounded-full bg-red-500 animate-pulse" />
              Disconnected
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {store.meta?.capabilities?.includes('query_mcp_status') && (
            <div className="p-0.5 rounded-md bg-gray-100 dark:bg-gray-800">
              <button
                type="button"
                onClick={() => setMcpPanelOpen((o) => !o)}
                className={[
                  'px-2.5 py-1 rounded text-sm transition-colors',
                  mcpPanelOpen
                    ? 'bg-white dark:bg-gray-700 text-gray-900 dark:text-gray-100 shadow-sm'
                    : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300',
                ].join(' ')}
              >
                MCP
              </button>
            </div>
          )}
          <ModeToggle mode={displayMode} onChange={handleModeChange} />
        </div>
      </div>

      {/* MCP Panel (collapsible) */}
      {mcpPanelOpen && store.meta?.capabilities?.includes('query_mcp_status') && (
        <div className="border-b border-gray-200 dark:border-gray-800">
          <McpPanel
            queryMcpStatus={() => channel?.request({ type: 'query_mcp_status' })}
            toggleMcp={(name, enabled) => channel?.send({ type: 'mcp_set', name, enabled })}
            reconnectMcp={(name) => channel?.send({ type: 'mcp_reconnect', name })}
          />
        </div>
      )}

      {/* Thread — all modes (chat, developer, watching) use ConversationThread.
          Developer mode uses developerRegistry for richer block rendering. */}
      <ExpandProvider>
        {/* Virtuoso manages its own scroll — no overflow-y-auto wrapper.
            flex-1 min-h-0 gives Virtuoso a measurable viewport height. */}
        <div className="flex-1 min-h-0 flex flex-col">
          {blocks.length === 0 ? (
            <div className="flex items-center justify-center flex-1 text-gray-400 dark:text-gray-500">
              <div className="text-center">
                <p className="text-lg font-medium mb-2">Start a conversation</p>
                <p className="text-sm mb-4">Send a message to begin.</p>
                {!sessionId && (
                  <div className="flex justify-center">
                    <ModelSelector
                      model={selectedModel}
                      onModelChange={handleModelChange}
                      isLive={viewMode === 'active'}
                    />
                  </div>
                )}
              </div>
            </div>
          ) : (
            <ConversationActionsProvider
              actions={{
                retryMessage: (localId) => dispatch({ type: 'RETRY_MESSAGE', localId }),
                stopTask: (taskId) => {
                  channel?.send({ type: 'stop_task', taskId })
                },
                respondPermission: (rid, allowed, perms) =>
                  dispatch({
                    type: 'RESPOND_PERMISSION',
                    requestId: rid,
                    allowed,
                    updatedPermissions: perms,
                  }),
                answerQuestion: (rid, answers) =>
                  dispatch({ type: 'ANSWER_QUESTION', requestId: rid, answers }),
                approvePlan: (rid, approved, feedback) =>
                  dispatch({ type: 'APPROVE_PLAN', requestId: rid, approved, feedback }),
                submitElicitation: (rid, response) =>
                  dispatch({ type: 'SUBMIT_ELICITATION', requestId: rid, response }),
              }}
            >
              <ConversationThread
                blocks={blocks}
                renderers={registry}
                filterBar={displayMode !== 'chat'}
              />
            </ConversationActionsProvider>
          )}
          {/* Connection status indicator — shows during acquiring/recovering phases */}
          {connectionStatus && (
            <div className="max-w-3xl mx-auto px-4 pb-3">
              {connectionStatus.kind === 'error' ? (
                <div className="flex items-center gap-2 text-sm text-red-500 dark:text-red-400">
                  <svg className="w-4 h-4 flex-shrink-0" viewBox="0 0 24 24" fill="none">
                    <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2" />
                    <line x1="8" y1="8" x2="16" y2="16" stroke="currentColor" strokeWidth="2" />
                    <line x1="16" y1="8" x2="8" y2="16" stroke="currentColor" strokeWidth="2" />
                  </svg>
                  {connectionStatus.message}
                </div>
              ) : (
                <div className="flex items-center gap-2 text-sm text-gray-500 dark:text-gray-400 animate-pulse">
                  <svg className="w-4 h-4 animate-spin" viewBox="0 0 24 24" fill="none">
                    <circle
                      className="opacity-25"
                      cx="12"
                      cy="12"
                      r="10"
                      stroke="currentColor"
                      strokeWidth="4"
                    />
                    <path
                      className="opacity-75"
                      fill="currentColor"
                      d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"
                    />
                  </svg>
                  {connectionStatus.message}
                </div>
              )}
            </div>
          )}
        </div>
      </ExpandProvider>

      {/* Input */}
      <div className="flex-shrink-0 border-t border-gray-200 dark:border-gray-800">
        <div className="max-w-3xl mx-auto px-4 py-3">
          {viewMode === 'watching' && (
            <div className="mb-2 rounded-lg border border-blue-200 dark:border-blue-800/50 bg-blue-50 dark:bg-blue-950/30 px-3 py-2">
              <p className="text-xs text-blue-600/80 dark:text-blue-400/70">
                This session is running in Claude Code CLI.
              </p>
              <button
                type="button"
                onClick={() => setShowTakeover(true)}
                className="mt-1 text-xs font-medium text-blue-600 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
              >
                Take Over
              </button>
            </div>
          )}
          <ChatInputBar
            onSend={handleSend}
            onStop={() => dispatch({ type: 'INTERRUPT' })}
            state={inputBar}
            mode={permMode}
            onModeChange={handleModeChangePermission}
            contextPercent={contextPercent}
            model={selectedModel}
            onModelChange={handleModelChange}
            effortValue={thinkingBudget}
            onEffortChange={
              store.meta?.capabilities?.includes('set_max_thinking_tokens')
                ? (tokens) => {
                    setThinkingBudget(tokens)
                    channel?.send({ type: 'set_max_thinking_tokens', tokens })
                  }
                : undefined
            }
            capabilities={
              store.meta
                ? {
                    model: store.meta.model,
                    permissionMode: store.meta.permissionMode as PermissionMode,
                    slashCommands: store.meta.slashCommands,
                    mcpServers: store.meta.mcpServers,
                    skills: store.meta.skills,
                    agents: store.meta.agents,
                  }
                : undefined
            }
            modelOptions={modelOptions}
            onModelSwitch={(newModel) => {
              setSelectedModel(newModel)
              try {
                localStorage.setItem(MODEL_STORAGE_KEY, newModel)
              } catch {
                /* noop */
              }
              channel?.send({ type: 'set_model', model: newModel })
            }}
            onPaletteModeChange={(newMode) =>
              dispatch({ type: 'RESUME_WITH_OPTIONS', permissionMode: newMode })
            }
            onCommand={(command) =>
              dispatch({ type: 'SEND_MESSAGE', text: `/${command}`, localId: crypto.randomUUID() })
            }
            onAgent={(agent) =>
              dispatch({ type: 'SEND_MESSAGE', text: `@${agent}`, localId: crypto.randomUUID() })
            }
            onPaletteOpen={() => {
              channel?.send({ type: 'query_commands' })
              channel?.send({ type: 'query_agents' })
            }}
          />
        </div>
      </div>

      <TakeoverDialog
        open={showTakeover}
        onConfirm={() => {
          setShowTakeover(false)
          dispatch({ type: 'TAKEOVER_CLI' })
        }}
        onCancel={() => setShowTakeover(false)}
      />
    </div>
  )
}
