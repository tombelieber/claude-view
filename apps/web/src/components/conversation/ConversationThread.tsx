import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { DayDivider, formatDayLabel } from './DayDivider'
import type { BlockRenderers } from './types'

interface Props {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
}

/** Extract unix-seconds timestamp from a block, if present. */
function getBlockTimestamp(block: ConversationBlock): number | undefined {
  if (block.type === 'user') return block.timestamp
  if (block.type === 'assistant') return block.timestamp
  return undefined
}

/** Get the calendar day string (YYYY-MM-DD) for grouping. */
function dayKey(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

export function ConversationThread({ blocks, renderers, compact }: Props) {
  let lastDay: string | null = null

  return (
    <div data-testid="message-thread" className={compact ? 'space-y-1' : 'space-y-3'}>
      {blocks.map((block) => {
        const Renderer = renderers[block.type]
        if (!Renderer) return null

        const ts = getBlockTimestamp(block)
        let divider: React.ReactNode = null

        if (ts && ts > 0) {
          const day = dayKey(ts)
          if (day !== lastDay) {
            lastDay = day
            divider = <DayDivider key={`day-${day}`} label={formatDayLabel(new Date(ts * 1000))} />
          }
        }

        return divider ? (
          <div key={block.id}>
            {divider}
            <Renderer block={block} />
          </div>
        ) : (
          <Renderer key={block.id} block={block} />
        )
      })}
    </div>
  )
}
