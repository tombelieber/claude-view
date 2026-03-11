import type { ConversationBlock } from '@claude-view/shared/types/blocks'

// Renderers accept their specific block subtype but are stored in a heterogeneous map.
// ConversationThread dispatches by type, so the union prop is safe at runtime.
export type BlockRenderer = React.ComponentType<{ block: ConversationBlock }>

export type BlockRenderers = Partial<Record<ConversationBlock['type'], BlockRenderer>>
