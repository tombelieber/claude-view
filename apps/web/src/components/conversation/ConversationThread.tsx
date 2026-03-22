import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import { ArrowDown, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import { cn } from '../../lib/utils'
import { type ChipDefinition, FilterChips } from '../live/action-log/FilterChips'
import { DayDivider, formatDayLabel } from './DayDivider'
import { DefaultExpandedProvider } from './blocks/developer/default-expanded-context'
import { JsonModeProvider } from './blocks/developer/json-mode-context'
import type { BlockRenderers } from './types'

// ── Fine-grained filter categories ──────────────────────────────────────────

export type FineCategory =
  | 'user'
  | 'assistant'
  | 'builtin'
  | 'mcp'
  | 'skill'
  | 'agent'
  | 'hook'
  | 'error'
  | 'system'
  | 'turn'
  | 'prompt'
  | 'queue'

const FINE_CATEGORIES: ChipDefinition<FineCategory>[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'user', label: 'User', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'assistant', label: 'Assistant', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'builtin', label: 'Builtin', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'mcp', label: 'MCP', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'skill', label: 'Skill', color: 'bg-purple-500/10 text-purple-400 border-purple-500/30' },
  { id: 'agent', label: 'Agent', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30' },
  { id: 'hook', label: 'Hook', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' },
  { id: 'error', label: 'Error', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
  { id: 'system', label: 'System', color: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30' },
  { id: 'turn', label: 'Turn', color: 'bg-green-500/10 text-green-400 border-green-500/30' },
  { id: 'prompt', label: 'Prompt', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' },
  { id: 'queue', label: 'Queue', color: 'bg-orange-500/10 text-orange-400 border-orange-500/30' },
]

/**
 * Derive one or more fine-grained categories for a block.
 * A block passes the filter if ANY of its categories match the active filter.
 */
export function getBlockFineCategories(block: ConversationBlock): FineCategory[] {
  switch (block.type) {
    case 'user':
      return ['user']

    case 'assistant': {
      const cats: FineCategory[] = []
      let hasText = false
      for (const seg of block.segments) {
        if (seg.kind === 'text') {
          hasText = true
        } else {
          // Map ActionCategory to FineCategory
          const tc = mapActionCategory(seg.execution.category)
          if (tc && !cats.includes(tc)) cats.push(tc)
        }
      }
      // If block has only text (no tools), it's "assistant"
      // If it has tools, include those categories; also include "assistant" if it has text
      if (cats.length === 0) return ['assistant']
      if (hasText) cats.push('assistant')
      return cats
    }

    case 'interaction':
      return ['prompt']

    case 'turn_boundary':
      return ['turn']

    case 'notice':
      return ['error']

    case 'system': {
      if (block.variant === 'hook_event') return ['hook']
      if (block.variant === 'queue_operation') return ['queue']
      return ['system']
    }

    case 'progress': {
      const pc = mapActionCategory(block.category)
      if (pc) return [pc]
      if (block.variant === 'hook') return ['hook']
      return ['system']
    }

    default:
      return ['system']
  }
}

function mapActionCategory(category: string | undefined): FineCategory | null {
  switch (category) {
    case 'builtin':
      return 'builtin'
    case 'mcp':
      return 'mcp'
    case 'skill':
      return 'skill'
    case 'agent':
      return 'agent'
    case 'hook':
      return 'hook'
    case 'queue':
      return 'queue'
    case 'snapshot':
      return 'system'
    case 'system':
      return 'system'
    default:
      return null
  }
}

function computeFineCounts(blocks: ConversationBlock[]): Record<FineCategory, number> {
  const c: Record<FineCategory, number> = {
    user: 0,
    assistant: 0,
    builtin: 0,
    mcp: 0,
    skill: 0,
    agent: 0,
    hook: 0,
    error: 0,
    system: 0,
    turn: 0,
    prompt: 0,
    queue: 0,
  }
  for (const block of blocks) {
    const cats = getBlockFineCategories(block)
    for (const cat of cats) c[cat]++
  }
  return c
}

// ── Flat item types for Virtuoso ────────────────────────────────────────────

type ThreadItem =
  | { kind: 'block'; block: ConversationBlock }
  | { kind: 'divider'; label: string; key: string }

function getBlockTimestamp(block: ConversationBlock): number | undefined {
  if (block.type === 'user') return block.timestamp
  if (block.type === 'assistant') return block.timestamp
  return undefined
}

function dayKey(unixSeconds: number): string {
  const d = new Date(unixSeconds * 1000)
  return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`
}

function buildThreadItems(blocks: ConversationBlock[]): ThreadItem[] {
  const items: ThreadItem[] = []
  let lastDay: string | null = null

  for (const block of blocks) {
    const ts = getBlockTimestamp(block)
    if (ts && ts > 0) {
      const day = dayKey(ts)
      if (day !== lastDay) {
        lastDay = day
        items.push({ kind: 'divider', label: formatDayLabel(new Date(ts * 1000)), key: day })
      }
    }
    items.push({ kind: 'block', block })
  }

  return items
}

// ── Component ───────────────────────────────────────────────────────────────

interface Props {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
  filterBar?: boolean
  /** Start with global JSON mode on (all cards show raw JSON). */
  defaultJsonMode?: boolean
  /** Start with all collapsible cards expanded (ToolCards, etc.). */
  defaultExpanded?: boolean
  /** Called when user scrolls to the top — triggers loading older messages. */
  onStartReached?: () => void
  /** Whether older messages are currently being fetched. */
  isFetchingOlder?: boolean
  /** Whether there are older messages available above. */
  hasOlderMessages?: boolean
}

export function ConversationThread({
  blocks,
  renderers,
  compact,
  filterBar,
  defaultJsonMode,
  defaultExpanded,
  onStartReached,
  isFetchingOlder,
  hasOlderMessages,
}: Props) {
  const [activeFilter, setActiveFilter] = useState<FineCategory[] | 'all'>('all')
  const [globalJsonMode, setGlobalJsonMode] = useState(defaultJsonMode ?? false)
  const counts = useMemo(() => computeFineCounts(blocks), [blocks])

  const visibleBlocks = useMemo(() => {
    if (activeFilter === 'all') return blocks
    return blocks.filter((b) => {
      const cats = getBlockFineCategories(b)
      return cats.some((c) => activeFilter.includes(c))
    })
  }, [blocks, activeFilter])

  const allItems = useMemo(() => buildThreadItems(visibleBlocks), [visibleBlocks])
  // Filter out items Virtuoso can't render — prevents "Zero-sized element" warnings and empty gaps.
  // Uses registry.canRender() for variant-level filtering (e.g. chat mode skips system/queue_operation).
  const items = useMemo(
    () =>
      allItems.filter((item) => {
        if (item.kind === 'divider') return true
        if (!renderers[item.block.type]) return false
        return renderers.canRender ? renderers.canRender(item.block) : true
      }),
    [allItems, renderers],
  )

  // ── Virtuoso scroll state ───────────────────────────────────────────────

  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [isAtBottom, setIsAtBottom] = useState(true)
  const [hasNewItems, setHasNewItems] = useState(false)
  const prevCountRef = useRef(items.length)
  const prevFilterRef = useRef(activeFilter)

  // ── Pagination: firstItemIndex for scroll-anchored prepending ─────────
  // Virtuoso uses firstItemIndex to maintain scroll position when items are
  // prepended. We start at a high number and decrease it as older pages load.
  const FIRST_INDEX = 100_000
  const [firstItemIndex, setFirstItemIndex] = useState(FIRST_INDEX)
  const wasFetchingOlderRef = useRef(false)
  const prevItemCountForPrepend = useRef(items.length)

  // Detect prepend completion: isFetchingOlder transitions true → false
  useLayoutEffect(() => {
    if (wasFetchingOlderRef.current && !isFetchingOlder) {
      const delta = items.length - prevItemCountForPrepend.current
      if (delta > 0) {
        setFirstItemIndex((prev) => prev - delta)
      }
    }
    wasFetchingOlderRef.current = !!isFetchingOlder
    prevItemCountForPrepend.current = items.length
  }, [items.length, isFetchingOlder])

  // Track when new items arrive while user is scrolled up
  useEffect(() => {
    if (items.length > prevCountRef.current) {
      if (!isAtBottom) {
        setHasNewItems(true)
      }
    }
    prevCountRef.current = items.length
  }, [items.length, isAtBottom])

  // Scroll to bottom when filter changes (list length changes drastically)
  useEffect(() => {
    if (prevFilterRef.current !== activeFilter) {
      prevFilterRef.current = activeFilter
      if (items.length > 0) {
        requestAnimationFrame(() => {
          virtuosoRef.current?.scrollToIndex({
            index: items.length - 1,
            behavior: 'auto',
          })
        })
      }
    }
  }, [activeFilter, items.length])

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    setIsAtBottom(atBottom)
    if (atBottom) {
      setHasNewItems(false)
    }
  }, [])

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({
      index: items.length - 1,
      align: 'end',
      behavior: 'smooth',
    })
    setHasNewItems(false)
  }, [items.length])

  // ── Filter handling ─────────────────────────────────────────────────────

  const handleFilterChange = (category: FineCategory | 'all') => {
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

  // ── Stable render callback ──────────────────────────────────────────────

  const renderItem = useCallback(
    (_index: number, item: ThreadItem) => {
      if (item.kind === 'divider') {
        return (
          <div className={compact ? 'py-0.5' : 'py-1 max-w-3xl mx-auto px-4'}>
            <DayDivider label={item.label} />
          </div>
        )
      }
      const Renderer = renderers[item.block.type]
      if (!Renderer) return null
      return (
        <div className={compact ? 'py-0.5 px-2' : 'py-1.5 max-w-3xl mx-auto px-4'}>
          <Renderer block={item.block} />
        </div>
      )
    },
    [renderers, compact],
  )

  const itemKey = useCallback(
    (_index: number, item: ThreadItem) =>
      item.kind === 'divider' ? `day-${item.key}` : item.block.id,
    [],
  )

  // ── Render ──────────────────────────────────────────────────────────────

  return (
    <DefaultExpandedProvider value={defaultExpanded ?? false}>
      <JsonModeProvider value={globalJsonMode}>
        <div data-testid="message-thread" className="relative h-full w-full flex flex-col">
          {filterBar && (
            <div className="sticky top-0 z-10 bg-white/80 dark:bg-gray-900/80 backdrop-blur-sm border-b border-gray-200/50 dark:border-gray-700/50 flex-shrink-0">
              <div className="flex items-center px-1 min-w-0">
                <div className="flex-1 min-w-0">
                  <FilterChips
                    categories={FINE_CATEGORIES}
                    counts={counts}
                    activeFilter={activeFilter}
                    onFilterChange={handleFilterChange}
                  />
                </div>
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

          {items.length === 0 ? (
            <div className="flex items-center justify-center flex-1 text-xs text-gray-500 dark:text-gray-600">
              No messages yet
            </div>
          ) : (
            <>
              {isFetchingOlder && (
                <div className="flex justify-center py-2 flex-shrink-0">
                  <Loader2 className="w-4 h-4 animate-spin text-gray-400" />
                </div>
              )}
              <Virtuoso
                ref={virtuosoRef}
                data={items}
                computeItemKey={itemKey}
                {...(onStartReached
                  ? { firstItemIndex, initialTopMostItemIndex: items.length - 1 }
                  : { initialTopMostItemIndex: items.length - 1 })}
                alignToBottom
                followOutput="smooth"
                atBottomStateChange={handleAtBottomStateChange}
                atBottomThreshold={30}
                itemContent={renderItem}
                startReached={
                  onStartReached && hasOlderMessages && !isFetchingOlder
                    ? onStartReached
                    : undefined
                }
                className="h-full flex-1 min-h-0"
              />
              {!isAtBottom && (
                <button
                  type="button"
                  onClick={scrollToBottom}
                  className={cn(
                    'absolute bottom-3 left-1/2 -translate-x-1/2 inline-flex items-center rounded-full shadow-lg transition-all cursor-pointer z-10',
                    hasNewItems
                      ? 'gap-1 px-3 py-1.5 bg-blue-600 text-white text-xs font-medium hover:bg-blue-500'
                      : 'p-2 bg-gray-800/80 text-gray-300 hover:bg-gray-700/80 backdrop-blur-sm',
                  )}
                >
                  <ArrowDown className="w-3.5 h-3.5" />
                  {hasNewItems && <span>New messages</span>}
                </button>
              )}
            </>
          )}
        </div>
      </JsonModeProvider>
    </DefaultExpandedProvider>
  )
}
