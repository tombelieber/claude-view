import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useMemo, useRef, useState } from 'react'
import { useHistoryBlocks } from './use-history-blocks'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

const SEND_TIMEOUT_MS = 10_000

export function useConversation(sessionId: string | undefined) {
  // Suppress 404 for sessions still initializing (JSONL not yet flushed).
  const source = useSessionSource(sessionId)
  const isInitializing = source.sessionState === 'initializing' || source.sessionState === 'active'
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
      return !source.blocks.some((b) => b.type === 'user' && (b as UserBlock).text === ob.text)
    })

    // Live overlay: stream blocks. Between turns this is empty.
    // During a turn this has the in-progress response.
    const liveOverlay = source.blocks

    return [...history.blocks, ...liveOverlay, ...pendingOptimistic]
  }, [history.blocks, source.blocks, optimisticBlocks])

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
