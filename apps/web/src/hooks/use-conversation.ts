import type { ConversationBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useMemo, useState } from 'react'
import { useInputState } from './use-input-state'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

const OPTIMISTIC_TIMEOUT_MS = 30_000

export function useConversation(sessionId: string | undefined) {
  const source = useSessionSource(sessionId)
  const actions = useSessionActions(source.send)
  const inputState = useInputState(source.sessionState, source.isLive)

  const [optimisticBlocks, setOptimisticBlocks] = useState<UserBlock[]>([])

  const sendMessage = useCallback(
    (text: string) => {
      const localId = crypto.randomUUID()
      const optimistic: UserBlock = {
        type: 'user',
        id: localId,
        localId,
        text,
        timestamp: Date.now() / 1000,
        status: 'optimistic',
      }
      setOptimisticBlocks((prev) => [...prev, optimistic])
      actions.sendMessage(text)

      const timer = setTimeout(() => {
        setOptimisticBlocks((prev) =>
          prev.map((b) => (b.localId === localId ? { ...b, status: 'failed' as const } : b)),
        )
      }, OPTIMISTIC_TIMEOUT_MS)

      return () => clearTimeout(timer)
    },
    [actions],
  )

  // Merge: remove optimistic blocks that appear in real blocks
  const blocks: ConversationBlock[] = useMemo(() => {
    const pendingOptimistic = optimisticBlocks.filter(
      (ob) =>
        !source.blocks.some((b) => b.type === 'user' && (b as UserBlock).localId === ob.localId),
    )
    return pendingOptimistic.length > 0 ? [...source.blocks, ...pendingOptimistic] : source.blocks
  }, [source.blocks, optimisticBlocks])

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
    inputState,
    actions: {
      ...actions,
      sendMessage,
      resume: source.resume,
      fork,
    },
    sessionInfo: {
      isLive: source.isLive,
      sessionState: source.sessionState,
      controlId: source.controlId,
    },
  }
}
