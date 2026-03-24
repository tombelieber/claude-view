import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useCallback, useMemo, useState } from 'react'
import { type ConnectionState, useTerminalSocket } from './use-terminal-socket'

/**
 * Merge a single block into a Map by ID — O(1) lookup.
 * Map preserves insertion order (ES2015 spec), so new blocks append naturally.
 */
export function mergeBlockById(
  map: Map<string, ConversationBlock>,
  incoming: ConversationBlock,
): Map<string, ConversationBlock> {
  const next = new Map(map)
  next.set(incoming.id, incoming)
  return next
}

export interface UseBlockSocketOptions {
  sessionId: string
  enabled: boolean
  /** Sub-agent path segment. If provided, connects to /subagents/{agentId}/terminal */
  agentId?: string
}

export interface UseBlockSocketResult {
  blocks: ConversationBlock[]
  bufferDone: boolean
  connectionState: ConnectionState
}

/**
 * Connects to Terminal WS in block mode and accumulates ConversationBlock[].
 * Each incoming message is a single ConversationBlock JSON; blocks are merged by ID.
 */
export function useBlockSocket({
  sessionId,
  enabled,
  agentId,
}: UseBlockSocketOptions): UseBlockSocketResult {
  const [blockMap, setBlockMap] = useState<Map<string, ConversationBlock>>(new Map())
  const [bufferDone, setBufferDone] = useState(false)

  const handleMessage = useCallback((data: string) => {
    try {
      const parsed = JSON.parse(data)
      if (parsed.type === 'buffer_end') {
        setBufferDone(true)
        return
      }
      if (parsed.type === 'pong' || parsed.type === 'error') return
      // Block mode: each message is a ConversationBlock with id + type
      if (parsed.id && parsed.type) {
        setBlockMap((prev) => mergeBlockById(prev, parsed as ConversationBlock))
      }
    } catch {
      // Not JSON — ignore
    }
  }, [])

  // Derive ordered array for render — Map preserves insertion order
  const blocks = useMemo(() => [...blockMap.values()], [blockMap])

  // Sub-agent path injection: prefix sessionId with /subagents/{agentId}
  const wsSessionId = agentId ? `${sessionId}/subagents/${agentId}` : sessionId

  // Use the REAL connectionState from useTerminalSocket — never fabricate it.
  const { connectionState } = useTerminalSocket({
    sessionId: wsSessionId,
    mode: 'block',
    scrollback: 50,
    enabled,
    onMessage: handleMessage,
  })

  return { blocks, bufferDone, connectionState }
}
