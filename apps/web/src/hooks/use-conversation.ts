import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useQueryClient } from '@tanstack/react-query'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useHistoryBlocks } from './use-history-blocks'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

const SEND_TIMEOUT_MS = 10_000

interface ConversationOptions {
  /** True when navigating from session creation — suppresses 404 during the
   *  race window before the JSONL file is flushed to disk. */
  freshlyCreated?: boolean
}

export function useConversation(sessionId: string | undefined, options?: ConversationOptions) {
  // Suppress 404 for sessions still initializing (JSONL not yet flushed).
  const source = useSessionSource(sessionId)
  // Freshly-created sessions start at 'idle' before WS connects and transitions
  // to 'initializing'/'active'. Suppress 404 during that gap so the messages
  // query doesn't enter permanent error state before the sidecar writes the JSONL.
  const isInitializing =
    source.sessionState === 'initializing' ||
    source.sessionState === 'active' ||
    (!!options?.freshlyCreated && source.sessionState === 'idle')
  const history = useHistoryBlocks(sessionId ?? null, {
    suppressNotFound: isInitializing,
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

  // History base + live overlay merge.
  // History = completed turns (from JSONL). Live = in-progress turn (from WS stream).
  // On turn_complete: turnVersion increments → invalidateQueries refetches → accumulator resets.
  const blocks: ConversationBlock[] = useMemo(() => {
    const pendingOptimistic = optimisticBlocks.filter((ob) => {
      const matchesText = (b: ConversationBlock) =>
        b.type === 'user' && (b as UserBlock).text === ob.text
      return !source.blocks.some(matchesText) && !history.blocks.some(matchesText)
    })

    // Live overlay: stream blocks. Between turns this is empty.
    // During a turn this has the in-progress response.
    const liveOverlay = source.blocks

    return [...history.blocks, ...liveOverlay, ...pendingOptimistic]
  }, [history.blocks, source.blocks, optimisticBlocks])

  const queryClient = useQueryClient()

  // Destructure for stable useEffect dependencies
  const { turnVersion, resetAccumulator } = source

  // On turn completion: invalidate history query (preserves cached pages + scroll),
  // then reset accumulator after history settles. Zero visual gap.
  const prevTurnVersionRef = useRef(0)
  useEffect(() => {
    if (turnVersion <= prevTurnVersionRef.current) return

    // Invalidate — triggers background refetch of active pages, no cache wipe
    queryClient.invalidateQueries({
      queryKey: ['session-messages', sessionId],
      refetchType: 'active',
    })
  }, [turnVersion, sessionId, queryClient])

  // Deferred accumulator reset: wait for history refetch to complete
  useEffect(() => {
    if (turnVersion <= prevTurnVersionRef.current) return
    if (history.isFetching) return // Still fetching — keep accumulator visible
    if (history.error) return // Fetch failed — DON'T reset, keep accumulator as fallback

    resetAccumulator()
    prevTurnVersionRef.current = turnVersion
  }, [turnVersion, history.isFetching, history.error, resetAccumulator])

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
      turnVersion: source.turnVersion,
      streamGap: source.streamGap,
    },
  }
}
