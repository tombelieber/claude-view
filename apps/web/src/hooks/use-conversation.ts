import type { ConversationBlock, NoticeBlock, UserBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useHistoryBlocks } from './use-history-blocks'
import { useSessionActions } from './use-session-actions'
import { useSessionSource } from './use-session-source'

const OPTIMISTIC_TIMEOUT_MS = 30_000

const RESUMED_DIVIDER: NoticeBlock = {
  type: 'notice',
  id: 'session-resumed-divider',
  variant: 'session_resumed',
  data: null,
}

export function useConversation(sessionId: string | undefined) {
  const history = useHistoryBlocks(sessionId ?? null)
  const source = useSessionSource(sessionId)
  const actions = useSessionActions(source.send)
  const canResumeLazy = source.canResumeLazy
  // NOTE: useInputState is NOT called here — each consumer (ChatPage, ConversationView,
  // SessionDetailPanel) calls deriveInputBarState() or useInputState() directly with
  // canResumeLazy from sessionInfo. This avoids a dead hook call.

  const [optimisticBlocks, setOptimisticBlocks] = useState<UserBlock[]>([])

  // Ref sync pattern: keep latest optimisticBlocks accessible in callbacks without stale closure
  const optimisticBlocksRef = useRef<UserBlock[]>([])
  optimisticBlocksRef.current = optimisticBlocks

  const sendMessage = useCallback(
    (text: string) => {
      // Lazy WS is handled inside source.send (effectiveSend) — it queues the message
      // and opens WS if needed, so no explicit connectIfNeeded() call required.

      const localId = crypto.randomUUID()
      const optimistic: UserBlock = {
        type: 'user',
        id: localId,
        localId,
        text,
        timestamp: Date.now() / 1000,
        status: canResumeLazy ? 'sending' : 'optimistic',
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
    [actions, canResumeLazy],
  )

  // Merge all block sources: history + divider + live + optimistic.
  // Dedup optimistic blocks that appear in EITHER history or live blocks.
  const blocks: ConversationBlock[] = useMemo(() => {
    const allRealBlocks = [...history.blocks, ...source.blocks]

    // Insert divider between history and live if both non-empty
    const merged =
      history.blocks.length > 0 && source.blocks.length > 0
        ? [...history.blocks, RESUMED_DIVIDER, ...source.blocks]
        : allRealBlocks

    // Remove optimistic blocks that have been confirmed by real blocks
    const pendingOptimistic = optimisticBlocks.filter(
      (ob) =>
        !allRealBlocks.some((b) => b.type === 'user' && (b as UserBlock).localId === ob.localId),
    )
    return pendingOptimistic.length > 0 ? [...merged, ...pendingOptimistic] : merged
  }, [history.blocks, source.blocks, optimisticBlocks])

  // Track previous isLive to detect the lazy-connect transition
  const prevIsLiveRef = useRef(source.isLive)

  // Transition 'sending' → 'sent' when WS connects (lazy connect completed)
  useEffect(() => {
    if (!prevIsLiveRef.current && source.isLive) {
      setOptimisticBlocks((prev) =>
        prev.map((ob) => (ob.status === 'sending' ? { ...ob, status: 'sent' as const } : ob)),
      )
    }
    prevIsLiveRef.current = source.isLive
  }, [source.isLive])

  // Remove 'sent' blocks after 500ms (the checkmark flash)
  useEffect(() => {
    const sentBlocks = optimisticBlocks.filter((ob) => ob.status === 'sent')
    if (sentBlocks.length === 0) return
    const timer = setTimeout(() => {
      setOptimisticBlocks((prev) => prev.filter((ob) => ob.status !== 'sent'))
    }, 500)
    return () => clearTimeout(timer)
  }, [optimisticBlocks])

  const retryMessage = useCallback(
    (localId: string) => {
      const failed = optimisticBlocksRef.current.find(
        (ob) => ob.localId === localId && ob.status === 'failed',
      )
      if (!failed) return
      setOptimisticBlocks((prev) => prev.filter((ob) => ob.localId !== localId))
      sendMessage(failed.text)
    },
    [sendMessage],
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
    // inputState removed — no consumer destructures it from useConversation.
    // ChatPage uses deriveInputBarState() directly, ConversationView does the same.
    // Each consumer that needs input state calls useInputState() or deriveInputBarState() directly.
    history, // NEW: expose for scroll-up pagination
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
    },
  }
}
