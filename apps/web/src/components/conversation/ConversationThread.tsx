import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { useMemo, useState } from 'react'
import { cn } from '../../lib/utils'
import { type ChipDefinition, FilterChips } from '../live/action-log/FilterChips'
import { DayDivider, formatDayLabel } from './DayDivider'
import { JsonModeProvider } from './blocks/developer/json-mode-context'
import type { BlockRenderers } from './types'

// ── Block-type categories for FilterChips ───────────────────────────────────

type BlockCategory = ConversationBlock['type']

const BLOCK_CATEGORIES: ChipDefinition<BlockCategory>[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'user', label: 'User', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'assistant', label: 'Assistant', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  {
    id: 'interaction',
    label: 'Prompt',
    color: 'bg-amber-500/10 text-amber-400 border-amber-500/30',
  },
  {
    id: 'turn_boundary',
    label: 'Turn',
    color: 'bg-green-500/10 text-green-400 border-green-500/30',
  },
  { id: 'notice', label: 'Notice', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
  { id: 'system', label: 'System', color: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30' },
  {
    id: 'progress',
    label: 'Progress',
    color: 'bg-amber-500/10 text-amber-400 border-amber-500/30',
  },
]

function computeCounts(blocks: ConversationBlock[]): Record<BlockCategory, number> {
  const c: Record<BlockCategory, number> = {
    user: 0,
    assistant: 0,
    interaction: 0,
    turn_boundary: 0,
    notice: 0,
    system: 0,
    progress: 0,
  }
  for (const block of blocks) c[block.type]++
  return c
}

// ── Component ───────────────────────────────────────────────────────────────

interface Props {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
  filterBar?: boolean
  /** Start with global JSON mode on (all cards show raw JSON). */
  defaultJsonMode?: boolean
}

function getBlockTimestamp(block: ConversationBlock): number | undefined {
  if (block.type === 'user') return block.timestamp
  if (block.type === 'assistant') return block.timestamp
  return undefined
}

function dayKey(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

export function ConversationThread({
  blocks,
  renderers,
  compact,
  filterBar,
  defaultJsonMode,
}: Props) {
  const [activeFilter, setActiveFilter] = useState<BlockCategory[] | 'all'>('all')
  const [globalJsonMode, setGlobalJsonMode] = useState(defaultJsonMode ?? false)
  const counts = useMemo(() => computeCounts(blocks), [blocks])

  const visibleBlocks = useMemo(() => {
    if (activeFilter === 'all') return blocks
    return blocks.filter((b) => activeFilter.includes(b.type))
  }, [blocks, activeFilter])

  const handleFilterChange = (category: BlockCategory | 'all') => {
    if (category === 'all') {
      setActiveFilter('all')
    } else {
      setActiveFilter((prev) => {
        if (prev === 'all') return [category]
        if (prev.includes(category)) {
          const next = prev.filter((c) => c !== category)
          return next.length === 0 ? 'all' : next
        }
        return [...prev, category]
      })
    }
  }

  let lastDay: string | null = null

  return (
    <JsonModeProvider value={globalJsonMode}>
      <div data-testid="message-thread" className={compact ? 'space-y-1' : 'space-y-3'}>
        {filterBar && (
          <div className="sticky top-0 z-10 bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm border-b border-gray-200/50 dark:border-gray-700/50 -mx-4 px-1">
            <div className="flex items-center">
              <FilterChips
                categories={BLOCK_CATEGORIES}
                counts={counts}
                activeFilter={activeFilter}
                onFilterChange={handleFilterChange}
              />
              <button
                onClick={() => setGlobalJsonMode((v) => !v)}
                className={cn(
                  'ml-auto mr-3 text-[10px] font-mono px-2 py-1 rounded-full border transition-colors duration-200 cursor-pointer flex-shrink-0',
                  globalJsonMode
                    ? 'text-amber-400 bg-amber-500/10 border-amber-500/30'
                    : 'text-gray-500 bg-transparent border-gray-700 hover:border-gray-600',
                )}
                title={globalJsonMode ? 'Switch all to Rich view' : 'Switch all to JSON view'}
              >
                {globalJsonMode ? '{ } JSON' : '{ }'}
              </button>
            </div>
          </div>
        )}
        {visibleBlocks.map((block) => {
          const Renderer = renderers[block.type]
          if (!Renderer) return null

          const ts = getBlockTimestamp(block)
          let divider: React.ReactNode = null

          if (ts && ts > 0) {
            const day = dayKey(ts)
            if (day !== lastDay) {
              lastDay = day
              divider = (
                <DayDivider key={`day-${day}`} label={formatDayLabel(new Date(ts * 1000))} />
              )
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
    </JsonModeProvider>
  )
}
