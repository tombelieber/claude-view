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
  const actions = useSessionActions(source.send, source.channel)
  // NOTE: useInputState is NOT called here — each consumer (ChatPage, ConversationView,
  // SessionDetailPanel) calls deriveInputBarState() or useInputState() directly with
  // canResumeLazy from sessionInfo. This avoids a dead hook call.

  const [optimisticBlocks, setOptimisticBlocks] = useState<UserBlock[]>([])

  // Ref sync pattern: keep latest optimisticBlocks accessible in callbacks without stale closure
  const optimisticBlocksRef = useRef<UserBlock[]>([])
  optimisticBlocksRef.current = optimisticBlocks

  const isLive = source.isLive
  const sendMessage = useCallback(
    (text: string) => {
      // Lazy WS / auto-resume is handled inside source.send (effectiveSend) — it queues
      // the message and opens WS (resuming if needed), so no explicit call required.

      const localId = crypto.randomUUID()
      const optimistic: UserBlock = {
        type: 'user',
        id: localId,
        localId,
        text,
        timestamp: Date.now() / 1000,
        // Live WS → 'sent' (message transmitted immediately)
        // Lazy/auto-resume → 'sending' (WS not yet open, will transition on connect)
        status: isLive ? 'sent' : 'sending',
      }
      setOptimisticBlocks((prev) => [...prev, optimistic])
      actions.sendMessage(text)

      const timer = setTimeout(() => {
        setOptimisticBlocks((prev) =>
          prev.map((b) => {
            if (b.localId !== localId) return b
            // Only fail blocks still waiting for confirmation
            if (b.status === 'optimistic' || b.status === 'sending') {
              return { ...b, status: 'failed' as const }
            }
            return b // Already 'sent' or cleared — don't override
          }),
        )
      }, OPTIMISTIC_TIMEOUT_MS)

      return () => clearTimeout(timer)
    },
    [actions, isLive],
  )

  // Merge all block sources: history + optimistic (user msgs) + stream (responses).
  // Optimistic blocks are placed BEFORE stream blocks so user messages appear
  // before the assistant's response — not appended at the end.
  const blocks: ConversationBlock[] = useMemo(() => {
    const allRealBlocks = [...history.blocks, ...source.blocks]

    // Dedup optimistic blocks confirmed by real blocks.
    // Active blocks: by localId. Confirmed (status cleared): by text + timestamp (2s window).
    const pendingOptimistic = optimisticBlocks.filter((ob) => {
      if (ob.status) {
        return !allRealBlocks.some(
          (b) => b.type === 'user' && (b as UserBlock).localId === ob.localId,
        )
      }
      return !allRealBlocks.some(
        (b) =>
          b.type === 'user' &&
          (b as UserBlock).text === ob.text &&
          Math.abs((b as UserBlock).timestamp - ob.timestamp) < 2,
      )
    })

    // Build timeline: history → [divider] → optimistic (user msgs) → stream (responses).
    // This ensures user messages appear BEFORE the assistant's response, not after.
    const liveSection = [...pendingOptimistic, ...source.blocks]

    if (liveSection.length === 0) return history.blocks
    if (history.blocks.length > 0) {
      return [...history.blocks, RESUMED_DIVIDER, ...liveSection]
    }
    return liveSection
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

  // Clear 'sent' status after 500ms (the checkmark flash).
  // Block stays in optimisticBlocks (keeps message visible) with status=undefined.
  useEffect(() => {
    const sentBlocks = optimisticBlocks.filter((ob) => ob.status === 'sent')
    if (sentBlocks.length === 0) return
    const timer = setTimeout(() => {
      setOptimisticBlocks((prev) =>
        prev.map((ob) => (ob.status === 'sent' ? { ...ob, status: undefined } : ob)),
      )
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
