import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { BlockRenderers } from './types'

interface Props {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
}

export function ConversationThread({ blocks, renderers, compact }: Props) {
  return (
    <div className={compact ? 'space-y-1' : 'space-y-3'}>
      {blocks.map((block) => {
        const Renderer = renderers[block.type]
        return Renderer ? <Renderer key={block.id} block={block} /> : null
      })}
    </div>
  )
}
