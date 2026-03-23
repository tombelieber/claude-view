import type { ConversationBlock } from '../../types/blocks'

// Renderers accept their specific block subtype but are stored in a heterogeneous map.
// ConversationThread dispatches by type, so the union prop is safe at runtime.
export type BlockRenderer = React.ComponentType<{ block: ConversationBlock }>

export type BlockRenderers = Partial<Record<ConversationBlock['type'], BlockRenderer>> & {
  /** Optional predicate — return false for blocks that exist in the registry
   *  but render nothing for certain variants (e.g. system/queue_operation in chat mode).
   *  ConversationThread filters these out before passing to Virtuoso. */
  canRender?: (block: ConversationBlock) => boolean
}
