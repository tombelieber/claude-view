import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'
import { ChatInputBar } from '../components/chat/ChatInputBar'
import { McpPanel } from '../components/chat/McpPanel'
import { ModelSelector } from '../components/chat/ModelSelector'
import { ThinkingBudgetControl } from '../components/chat/ThinkingBudgetControl'
import { ConversationThread } from '../components/conversation/ConversationThread'
import { chatRegistry } from '../components/conversation/blocks/chat/registry'
import { developerRegistry } from '../components/conversation/blocks/developer/registry'

import { ConversationActionsProvider } from '../contexts/conversation-actions-context'
import { useConversation } from '../hooks/use-conversation'
import { resolveSessionModel, useModelOptions } from '../hooks/use-models'
import { useRichSessionData } from '../hooks/use-rich-session-data'
import { useScrollAnchor } from '../hooks/use-scroll-anchor'
import { useSessionCapabilities } from '../hooks/use-session-capabilities'
import { useSessionDetail } from '../hooks/use-session-detail'
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

/** Authoritative context data from Live Monitor SSE (statusline source). */
interface LiveContextData {
  contextWindowTokens: number
  statuslineContextWindowSize: number | null
  statuslineUsedPct: number | null
}

interface ChatSessionProps {
  sessionId: string | undefined
  /** True when session is live elsewhere (CLI/VS Code) but NOT sidecar-managed. */
  isWatching?: boolean
  /** Authoritative context gauge data from Live Monitor SSE. Undefined when no live session. */
  liveContextData?: LiveContextData
}

export function ChatSession({ sessionId, isWatching, liveContextData }: ChatSessionProps) {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  // When watching, skip WS to prevent auto-resume/bind_control. History loads via REST.
  const { blocks, history, actions, sessionInfo } = useConversation(sessionId, {
    skipWs: isWatching,
  })
  const { data: richData } = useRichSessionData(sessionId || null)
  const { data: sessionDetail } = useSessionDetail(sessionId || null)

  const { scrollContainerRef, topSentinelRef, bottomRef, handleScroll } = useScrollAnchor({
    onReachTop: history.hasOlderMessages ? history.fetchOlderMessages : undefined,
    isFetchingOlder: history.isFetchingOlder,
    blockCount: blocks.length,
  })

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
  const inputBarState = deriveInputBarState(
    sessionInfo.sessionState,
    sessionInfo.isLive,
    sessionInfo.canResumeLazy,
  )

  // Permission mode — persisted globally (like model), applied at session creation/resume
  const MODE_STORAGE_KEY = 'claude-view:last-mode'
  const [permMode, setPermMode] = useState<PermissionMode>(() => {
    try {
      const stored = localStorage.getItem(MODE_STORAGE_KEY) as PermissionMode | null
      return stored ?? 'default'
    } catch {
      return 'default'
    }
  })

  // Context gauge — priority chain (most authoritative first, never show wrong data):
  //  1. statuslineUsedPct: pre-computed by Claude Code — correct numerator AND denominator
  //  2. Live Monitor contextWindowTokens + statuslineContextWindowSize: correct per-turn fill
  //  3. richData contextWindowTokens: JSONL accumulator (history), correct semantics
  //  4. undefined: show "--" — refuse to guess
  // NOTE: WS totalInputTokens is intentionally NOT used here — it sums across all models
  // in modelUsage and may be session-cumulative, producing inflated percentages (84% vs 12%).
  const contextPercent = (() => {
    if (liveContextData?.statuslineUsedPct != null) {
      return Math.round(liveContextData.statuslineUsedPct)
    }
    if (liveContextData && liveContextData.contextWindowTokens > 0) {
      const limit = getContextLimit(
        null,
        liveContextData.contextWindowTokens,
        liveContextData.statuslineContextWindowSize,
      )
      return Math.round((liveContextData.contextWindowTokens / limit) * 100)
    }
    if (richData && richData.contextWindowTokens > 0) {
      const limit = getContextLimit(null, richData.contextWindowTokens)
      return Math.round((richData.contextWindowTokens / limit) * 100)
    }
    return undefined
  })()

  const handleSend = useCallback(
    (text: string) => {
      if (!sessionId) {
        // No session yet — create one first, then navigate.
        // initialMessage is echoed back via user_message_echo in the stream,
        // so the user sees their message as soon as the WS connects.
        fetch('/api/control/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            model: selectedModel,
            initialMessage: text,
            permissionMode: permMode,
          }),
        })
          .then((r) => r.json())
          .then((data) => {
            if (data.sessionId) {
              navigate(`/chat/${data.sessionId}`)
              queryClient.invalidateQueries({ queryKey: ['sidecar-sessions'] })
            } else {
              toast.error('Failed to create session', {
                description: data.error || 'No session ID returned',
              })
            }
          })
          .catch(() => {
            toast.error('Failed to create session')
          })
        return
      }
      actions.sendMessage(text)
    },
    [sessionId, actions, navigate, selectedModel, permMode, queryClient],
  )

  const handleModeChangePermission = useCallback(
    (mode: PermissionMode) => {
      setPermMode(mode)
      try {
        localStorage.setItem(MODE_STORAGE_KEY, mode)
      } catch {
        /* noop */
      }
      // Push to sidecar if live (sendIfLive no-ops if dormant).
      // bypassPermissions will fail mid-session via setPermissionMode but the
      // sidecar falls back to close+re-resume internally.
      actions.setPermissionMode(mode)
    },
    [actions],
  )

  // --- Command palette ---
  const capabilities = useSessionCapabilities(sessionInfo)
  const { options: modelOptions } = useModelOptions()

  // History session: auto-select the session's primary model if SDK-supported,
  // otherwise keep the user's default (from localStorage).
  useEffect(() => {
    if (!sessionId || !sessionDetail?.primaryModel || modelOptions.length === 0) return
    const resolved = resolveSessionModel(sessionDetail.primaryModel, modelOptions)
    if (resolved) setSelectedModel(resolved)
  }, [sessionId, sessionDetail?.primaryModel, modelOptions])

  const handleModelSwitch = useCallback(
    (newModel: string) => {
      toast('Switch model?', {
        description: `Re-ingests ~${Math.round(sessionInfo.totalInputTokens / 1000)}K context tokens.`,
        action: {
          label: 'Switch',
          onClick: () => {
            actions.resume(capabilities.permissionMode, newModel).catch(() => {
              toast.error('Failed to switch model', {
                description: 'Session may need manual resume.',
              })
            })
          },
        },
      })
    },
    [actions, capabilities.permissionMode, sessionInfo.totalInputTokens],
  )

  const handlePaletteModeChange = useCallback(
    (newMode: PermissionMode) => {
      toast('Change permissions?', {
        description: `Re-ingests ~${Math.round(sessionInfo.totalInputTokens / 1000)}K context tokens.`,
        action: {
          label: 'Change',
          onClick: () => {
            actions.resume(newMode, capabilities.model).catch(() => {
              toast.error('Failed to change permissions', {
                description: 'Session may need manual resume.',
              })
            })
          },
        },
      })
    },
    [actions, capabilities.model, sessionInfo.totalInputTokens],
  )

  const handlePaletteCommand = useCallback(
    (command: string) => {
      actions.sendMessage(`/${command}`)
    },
    [actions],
  )

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-gray-200 dark:border-gray-800 flex-shrink-0">
        <div className="flex items-center gap-3">
          {isWatching ? (
            <span className="flex items-center gap-1.5 text-xs text-blue-500">
              <span className="w-1.5 h-1.5 rounded-full bg-blue-500 animate-pulse" />
              Watching
            </span>
          ) : sessionInfo.isLive ? (
            <span className="flex items-center gap-1.5 text-xs text-green-500">
              <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
              Live
            </span>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {sessionInfo.capabilities?.includes('set_max_thinking_tokens') && (
            <ThinkingBudgetControl
              value={thinkingBudget}
              onChange={(tokens) => {
                setThinkingBudget(tokens)
                actions.setMaxThinkingTokens(tokens)
              }}
              disabled={!sessionInfo.isLive}
            />
          )}
          {sessionInfo.capabilities?.includes('query_mcp_status') && (
            <button
              type="button"
              onClick={() => setMcpPanelOpen((o) => !o)}
              className="text-xs px-2 py-1 rounded border border-gray-200 dark:border-gray-700 hover:bg-gray-50 dark:hover:bg-gray-800"
            >
              MCP
            </button>
          )}
          <ModeToggle mode={displayMode} onChange={handleModeChange} />
        </div>
      </div>

      {/* MCP Panel (collapsible) */}
      {mcpPanelOpen && sessionInfo.capabilities?.includes('query_mcp_status') && (
        <div className="border-b border-gray-200 dark:border-gray-800">
          <McpPanel
            queryMcpStatus={() => actions.queryMcpStatus()}
            toggleMcp={(name, enabled) => actions.toggleMcp(name, enabled)}
            reconnectMcp={(name) => actions.reconnectMcp(name)}
          />
        </div>
      )}

      {/* Thread */}
      <div ref={scrollContainerRef} onScroll={handleScroll} className="flex-1 overflow-y-auto">
        {/* Top sentinel for infinite scroll */}
        <div ref={topSentinelRef} className="h-1" />
        {history.isFetchingOlder && (
          <div className="flex justify-center py-3">
            <div className="h-5 w-5 animate-spin rounded-full border-2 border-gray-300 border-t-blue-500" />
          </div>
        )}
        {history.error && (
          <div className="flex justify-center py-3 text-sm text-red-500">
            Failed to load messages.{' '}
            <button type="button" onClick={history.fetchOlderMessages} className="underline">
              Retry
            </button>
          </div>
        )}
        {blocks.length === 0 ? (
          <div className="flex items-center justify-center h-full text-gray-400 dark:text-gray-500">
            <div className="text-center">
              <p className="text-lg font-medium mb-2">Start a conversation</p>
              <p className="text-sm mb-4">Send a message to begin.</p>
              {!sessionId && (
                <div className="flex justify-center">
                  <ModelSelector
                    model={selectedModel}
                    onModelChange={handleModelChange}
                    isLive={sessionInfo.isLive}
                    onSetModel={actions.setModel}
                  />
                </div>
              )}
            </div>
          </div>
        ) : (
          <div className="max-w-3xl mx-auto px-4 py-6">
            <ConversationActionsProvider
              actions={{
                retryMessage: actions.retryMessage,
                stopTask: actions.stopTask,
                respondPermission: actions.respondPermission,
                answerQuestion: actions.answerQuestion,
                approvePlan: actions.approvePlan,
                submitElicitation: actions.submitElicitation,
              }}
            >
              <ConversationThread blocks={blocks} renderers={registry} />
            </ConversationActionsProvider>
          </div>
        )}
        {/* Bottom anchor for auto-scroll */}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="flex-shrink-0 border-t border-gray-200 dark:border-gray-800">
        <div className="max-w-3xl mx-auto px-4 py-3">
          {isWatching && (
            <div className="mb-2 rounded-lg border border-blue-200 dark:border-blue-800/50 bg-blue-50 dark:bg-blue-950/30 px-4 py-3">
              <div className="flex items-start gap-3">
                <span className="mt-0.5 text-blue-500 dark:text-blue-400 text-base">&#x1f441;</span>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-blue-800 dark:text-blue-300">
                    Watching a live session
                  </p>
                  <p className="text-xs text-blue-600/70 dark:text-blue-400/60 mt-0.5">
                    This session is running in another process. Take over and resume from Claude
                    View is coming soon.
                  </p>
                </div>
              </div>
            </div>
          )}
          <ChatInputBar
            onSend={handleSend}
            onStop={actions.interrupt}
            state={isWatching ? 'controlled_elsewhere' : inputBarState}
            mode={permMode}
            onModeChange={handleModeChangePermission}
            contextPercent={contextPercent}
            model={selectedModel}
            onModelChange={handleModelChange}
            capabilities={capabilities}
            modelOptions={modelOptions}
            onModelSwitch={handleModelSwitch}
            onPaletteModeChange={handlePaletteModeChange}
            onCommand={handlePaletteCommand}
            onAgent={(agent) => actions.sendMessage(`@${agent}`)}
            onPaletteOpen={() => {
              actions.queryCommands().catch(() => {})
              actions.queryAgents().catch(() => {})
            }}
          />
        </div>
      </div>
    </div>
  )
}
