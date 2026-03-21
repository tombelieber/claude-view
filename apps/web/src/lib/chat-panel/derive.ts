import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { ChatPanelStore, InputBarState, ViewMode } from './types'

// ─── Helpers ───────────────────────────────────────────────

function getBaseBlocks(store: ChatPanelStore): ConversationBlock[] {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return []
    case 'nobody':
      return panel.sub.sub === 'ready' ? panel.sub.blocks : []
    case 'cc_cli':
      return []
    case 'acquiring':
      return panel.historyBlocks
    case 'sdk_owned': {
      const blocks = [...panel.blocks]
      if (panel.pendingText) {
        blocks.push({
          type: 'assistant',
          id: '__pending__',
          segments: [{ kind: 'text', text: panel.pendingText }],
          streaming: true,
          timestamp: Date.now(),
        })
      }
      return blocks
    }
    case 'recovering':
      return panel.blocks
    case 'closed':
      return panel.blocks
  }
}

function reconcileOutbox(blocks: ConversationBlock[], store: ChatPanelStore): ConversationBlock[] {
  const { outbox } = store
  if (outbox.messages.length === 0) return blocks

  const existingTexts = new Set(
    blocks
      .filter((b): b is ConversationBlock & { type: 'user' } => b.type === 'user')
      .map((b) => b.text),
  )

  const synthetic: ConversationBlock[] = []
  for (const entry of outbox.messages) {
    if (!existingTexts.has(entry.text)) {
      synthetic.push({
        type: 'user',
        id: `outbox-${entry.localId}`,
        text: entry.text,
        timestamp: Date.now(),
        localId: entry.localId,
        status: entry.status === 'failed' ? 'failed' : 'optimistic',
      } as ConversationBlock)
    }
  }

  return synthetic.length > 0 ? [...blocks, ...synthetic] : blocks
}

// ─── Public API ────────────────────────────────────────────

export function deriveBlocks(store: ChatPanelStore): ConversationBlock[] {
  const blocks = getBaseBlocks(store)
  return reconcileOutbox(blocks, store)
}

export function deriveCanSend(store: ChatPanelStore): boolean {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return true
    case 'nobody':
      return panel.sub.sub === 'ready'
    case 'cc_cli':
      return panel.sub.sub === 'watching'
    case 'acquiring':
      return false
    case 'sdk_owned':
      return panel.turn.turn === 'idle'
    case 'recovering':
      return true
    case 'closed':
      return !panel.ephemeral
  }
}

export function deriveCanFork(store: ChatPanelStore): boolean {
  const { panel } = store
  switch (panel.phase) {
    case 'nobody':
      return panel.sub.sub === 'ready' && panel.sub.blocks.length > 0
    case 'sdk_owned':
      return panel.blocks.length > 0
    default:
      return false
  }
}

export function deriveInputBar(store: ChatPanelStore): InputBarState {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return 'dormant'
    case 'nobody':
      return 'active'
    case 'cc_cli':
      return 'controlled_elsewhere'
    case 'acquiring':
      return 'connecting'
    case 'sdk_owned': {
      if (panel.conn.health === 'reconnecting') return 'reconnecting'
      switch (panel.turn.turn) {
        case 'idle':
          return 'active'
        case 'streaming':
          return 'streaming'
        case 'awaiting':
          return 'waiting_permission'
        case 'compacting':
          return 'streaming'
      }
      break
    }
    case 'recovering':
      return 'active'
    case 'closed':
      return 'completed'
  }
}

export function deriveViewMode(store: ChatPanelStore): ViewMode {
  const { panel } = store
  switch (panel.phase) {
    case 'empty':
      return 'blank'
    case 'nobody':
      return panel.sub.sub === 'loading' ? 'loading' : 'history'
    case 'cc_cli':
      return 'watching'
    case 'acquiring':
      return 'connecting'
    case 'sdk_owned':
      return 'active'
    case 'recovering':
      return 'error'
    case 'closed':
      return 'closed'
  }
}
