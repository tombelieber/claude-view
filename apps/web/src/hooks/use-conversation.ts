import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { appendPendingText } from './append-pending-text'
import { useHistoryBlocks } from './use-history-blocks'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

const SEND_TIMEOUT_MS = 10_000

interface ConversationOptions {
  /** Skip WS connection (watching mode). History still loads via REST. */
  skipWs?: boolean
}

export function useConversation(sessionId: string | undefined, options?: ConversationOptions) {
  // 'skipWs' allows watching mode: skip WS connection (no bind_control)
  // while still loading history with the real sessionId.
  const source = useSessionSource(options?.skipWs ? undefined : sessionId)

  // Gate history fetching with initComplete — prevents 404 error banner on new sessions.
  // Before init() resolves, the JSONL may not exist yet. After init(), either:
  // - Session is live → blocks come from committedBlocks via sidecar WS (enabled=false)
  // - Session is history-only → JSONL exists, safe to fetch (enabled=true)
  const isInitializing = source.sessionState === 'initializing'
  const history = useHistoryBlocks(sessionId ?? null, {
    enabled: (options?.skipWs || source.initComplete) && !source.isLive && !!sessionId,
    suppressNotFound: isInitializing,
    retry: 3,
    retryDelay: 1000,
  })
  const actions = useSessionActions(source.send, source.sendIfLive, source.channel)

  const [optimisticBlocks, setOptimisticBlocks] = useState<UserBlock[]>([])
  const optimisticBlocksRef = useRef<UserBlock[]>([])
  optimisticBlocksRef.current = optimisticBlocks

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

      const timer = setTimeout(() => {
        setOptimisticBlocks((prev) =>
          prev.map((b) => {
            if (b.localId !== localId) return b
            if (b.status === 'sending') return { ...b, status: 'failed' as const }
            return b
          }),
        )
      }, SEND_TIMEOUT_MS)

      return () => clearTimeout(timer)
    },
    [actions],
  )

  const queryClient = useQueryClient()

  // FLAG-B backward compat: if isLive but no blocks_snapshot arrives within 3s,
  // fall back to JSONL history (sidecar hasn't been upgraded to send snapshots yet).
  const [snapshotTimeout, setSnapshotTimeout] = useState(false)

  useEffect(() => {
    if (!source.isLive) {
      setSnapshotTimeout(false)
      return
    }
    const t = setTimeout(() => {
      if (source.committedBlocks.length === 0) setSnapshotTimeout(true)
    }, 3000)
    return () => clearTimeout(t)
  }, [source.isLive, source.committedBlocks.length])

  // Binary source switch: live (sidecar WS) vs history (JSONL).
  // When live and snapshot received, use committed blocks + pending text.
  // When not live (or snapshot timeout), use JSONL history.
  const blocks: ConversationBlock[] = useMemo(() => {
    const base = source.isLive && !snapshotTimeout ? source.committedBlocks : history.blocks
    const withPending = appendPendingText(base, source.pendingText)
    const pendingOptimistic = optimisticBlocks.filter((ob) => {
      const matchesText = (b: ConversationBlock) =>
        b.type === 'user' && (b as UserBlock).text === ob.text
      return !withPending.some(matchesText)
    })
    return [...withPending, ...pendingOptimistic]
  }, [
    source.isLive,
    snapshotTimeout,
    source.committedBlocks,
    source.pendingText,
    history.blocks,
    optimisticBlocks,
  ])

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
    const res = await fetch('/api/control/sessions/fork', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ sessionId }),
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
      canResumeLazy: source.canResumeLazy,
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
