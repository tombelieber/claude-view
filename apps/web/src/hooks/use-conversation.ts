import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { appendPendingText } from './append-pending-text'
import { useHistoryBlocks } from './use-history-blocks'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

// Dormant sessions auto-resume via connectAndSend → POST resume → waitForSessionInit (up to 15s).
// Timeout must exceed the resume window to avoid false "Failed" during normal resume flow.
const SEND_TIMEOUT_MS = 20_000

import type { LiveStatus } from '../lib/derive-panel-mode'

interface ConversationOptions {
  liveStatus?: LiveStatus
}

export function useConversation(sessionId: string | undefined, options?: ConversationOptions) {
  // cc_owned = watching mode: skip WS connection (no bind_control)
  // while still loading history with the real sessionId.
  const source = useSessionSource(options?.liveStatus === 'cc_owned' ? undefined : sessionId)

  // Gate history fetching with initComplete — prevents 404 error banner on new sessions.
  // Before init() resolves, the JSONL may not exist yet. After init(), either:
  // - Session is live → blocks come from committedBlocks via sidecar WS (enabled=false)
  // - Session is history-only → JSONL exists, safe to fetch (enabled=true)
  const isInitializing = source.sessionState === 'initializing'
  const history = useHistoryBlocks(sessionId ?? null, {
    enabled:
      (options?.liveStatus === 'cc_owned' || source.initComplete) && !source.isLive && !!sessionId,
    suppressNotFound: isInitializing,
    retry: 3,
    retryDelay: 1000,
  })
  const actions = useSessionActions(source.send, source.sendIfLive, source.channel)

  const [optimisticBlocks, setOptimisticBlocks] = useState<UserBlock[]>([])
  const optimisticBlocksRef = useRef<UserBlock[]>([])
  optimisticBlocksRef.current = optimisticBlocks

  // Deferred timer refs: messages sent while dormant (isLive=false) defer their
  // fail timer until the WS connection opens (isLive transitions to true).
  const pendingTimersRef = useRef<Map<string, number>>(new Map())
  const activeTimersRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())
  const isLiveRef = useRef(source.isLive)
  isLiveRef.current = source.isLive

  const startFailTimer = useCallback((localId: string) => {
    const timer = setTimeout(() => {
      setOptimisticBlocks((prev) =>
        prev.map((b) => {
          if (b.localId !== localId) return b
          if (b.status === 'sending') return { ...b, status: 'failed' as const }
          return b
        }),
      )
      activeTimersRef.current.delete(localId)
    }, SEND_TIMEOUT_MS)
    activeTimersRef.current.set(localId, timer)
  }, [])

  const sendMessage = useCallback(
    (text: string) => {
      const localId = crypto.randomUUID()
      const optimistic: UserBlock = {
        type: 'user',
        id: localId,
        localId,
        text,
        timestamp: Date.now() / 1000,
        status: 'sending',
      }
      setOptimisticBlocks((prev) => [...prev, optimistic])
      actions.sendMessage(text)

      if (isLiveRef.current) {
        // WS already open — start timer immediately
        startFailTimer(localId)
      } else {
        // WS not yet open — defer timer until connection establishes
        pendingTimersRef.current.set(localId, Date.now())
      }
    },
    [actions, startFailTimer],
  )

  // Fire deferred timers when WS opens (isLive transitions false → true)
  useEffect(() => {
    if (!source.isLive || pendingTimersRef.current.size === 0) return
    for (const [localId] of pendingTimersRef.current) {
      startFailTimer(localId)
    }
    pendingTimersRef.current.clear()
  }, [source.isLive, startFailTimer])

  // Cleanup active timers on unmount
  useEffect(() => {
    return () => {
      for (const timer of activeTimersRef.current.values()) {
        clearTimeout(timer)
      }
    }
  }, [])

  const queryClient = useQueryClient()

  // Source switch: use live committedBlocks when the sidecar has content,
  // otherwise use JSONL history. pendingText only applies to live blocks
  // (never appended to history — that causes "response appends to last message" bug).
  //
  // No timeout fallback — the switch is purely content-based:
  //   committedBlocks.length > 0 → sidecar has caught up, use it
  //   committedBlocks.length === 0 → sidecar hasn't caught up, use history
  const blocks: ConversationBlock[] = useMemo(() => {
    const hasLiveBlocks = source.isLive && source.committedBlocks.length > 0
    const base = hasLiveBlocks ? source.committedBlocks : history.blocks
    const withPending = hasLiveBlocks ? appendPendingText(base, source.pendingText) : base
    const pendingOptimistic = optimisticBlocks.filter((ob) => {
      const matchesText = (b: ConversationBlock) =>
        b.type === 'user' && (b as UserBlock).text === ob.text
      return !withPending.some(matchesText)
    })
    return [...withPending, ...pendingOptimistic]
  }, [source.isLive, source.committedBlocks, source.pendingText, history.blocks, optimisticBlocks])

  // isLive->false transition: invalidate JSONL cache so history refetches
  useEffect(() => {
    if (!source.isLive && sessionId) {
      queryClient.invalidateQueries({ queryKey: ['session-messages', sessionId] })
    }
  }, [source.isLive, sessionId, queryClient])

  const retryMessage = useCallback(
    (localId: string) => {
      const failed = optimisticBlocksRef.current.find(
        (ob) => ob.localId === localId && ob.status === 'failed',
      )
      if (!failed) return
      setOptimisticBlocks((prev) => prev.filter((ob) => ob.localId !== localId))
      source.clearPendingMessage(failed.text)
      sendMessage(failed.text)
    },
    [sendMessage, source],
  )

  const fork = useCallback(async () => {
    if (!sessionId) return null
    const res = await fetch(`/api/sidecar/sessions/${sessionId}/fork`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({}),
    })
    return res.json()
  }, [sessionId])

  return {
    blocks,
    history,
    actions: {
      ...actions,
      sendMessage,
      retryMessage,
      resume: source.resume,
      fork,
    },
    sessionInfo: {
      isLive: source.isLive,
      sessionState: source.sessionState,
      controlId: source.controlId,
      totalInputTokens: source.totalInputTokens,
      contextWindowSize: source.contextWindowSize,
      model: source.model,
      slashCommands: source.slashCommands,
      mcpServers: source.mcpServers,
      permissionMode: source.permissionMode,
      skills: source.skills,
      agents: source.agents,
      capabilities: source.capabilities,
    },
  }
}
