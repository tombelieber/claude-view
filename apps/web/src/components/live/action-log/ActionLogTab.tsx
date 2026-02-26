import { ArrowDown } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Virtuoso, type VirtuosoHandle } from 'react-virtuoso'
import type { RichMessage } from '../RichPane'
import { ActionFilterChips } from './ActionFilterChips'
import { ActionRow } from './ActionRow'
import { TurnSeparatorRow } from './TurnSeparatorRow'
import { isHookEvent, isTurnSeparator } from './types'
import type { ActionCategory } from './types'
import { useActionItems } from './use-action-items'

interface ActionLogTabProps {
  messages: RichMessage[]
  bufferDone: boolean
  /** Pre-computed category counts from canonical message array. */
  categoryCounts?: Record<ActionCategory, number>
}

export function ActionLogTab({
  messages,
  bufferDone,
  categoryCounts: countsProp,
}: ActionLogTabProps) {
  const allItems = useActionItems(messages)
  const [activeFilter, setActiveFilter] = useState<ActionCategory | 'all'>('all')
  const virtuosoRef = useRef<VirtuosoHandle>(null)
  const [atBottom, setAtBottom] = useState(true)
  const [showNewIndicator, setShowNewIndicator] = useState(false)
  const prevCountRef = useRef(0)

  // Use prop if provided, otherwise compute from allItems (backward compat)
  const counts = useMemo(() => {
    if (countsProp) return countsProp
    const c: Record<ActionCategory, number> = {
      skill: 0,
      mcp: 0,
      builtin: 0,
      agent: 0,
      error: 0,
      hook: 0,
      hook_progress: 0,
      system: 0,
      snapshot: 0,
      queue: 0,
      context: 0,
      result: 0,
      summary: 0,
    }
    for (const item of allItems) {
      if (!isTurnSeparator(item) && !isHookEvent(item)) {
        c[item.category]++
      }
    }
    return c
  }, [countsProp, allItems])

  // Filtered items
  const filteredItems = useMemo(() => {
    if (activeFilter === 'all') return allItems
    return allItems.filter((item) => {
      if (isTurnSeparator(item)) return true // always show turn separators
      if (isHookEvent(item)) return activeFilter === 'hook'
      return item.category === activeFilter
    })
  }, [allItems, activeFilter])

  // Show "new actions" indicator when not at bottom
  useEffect(() => {
    if (filteredItems.length > prevCountRef.current && !atBottom) {
      setShowNewIndicator(true)
    }
    prevCountRef.current = filteredItems.length
  }, [filteredItems.length, atBottom])

  // Scroll to bottom on bufferDone
  useEffect(() => {
    if (bufferDone && virtuosoRef.current) {
      requestAnimationFrame(() => {
        virtuosoRef.current?.scrollToIndex({ index: filteredItems.length - 1, behavior: 'auto' })
      })
    }
  }, [bufferDone]) // intentionally exclude filteredItems.length to avoid loop

  const scrollToBottom = useCallback(() => {
    virtuosoRef.current?.scrollToIndex({ index: filteredItems.length - 1, behavior: 'smooth' })
    setShowNewIndicator(false)
  }, [filteredItems.length])

  return (
    <div className="flex flex-col h-full">
      {/* Filter chips */}
      <ActionFilterChips
        counts={counts}
        activeFilter={activeFilter}
        onFilterChange={setActiveFilter}
      />

      {/* Timeline */}
      <div className="flex-1 min-h-0 relative">
        {filteredItems.length === 0 ? (
          <div className="flex items-center justify-center h-full text-sm text-gray-500">
            No actions yet
          </div>
        ) : (
          <Virtuoso
            ref={virtuosoRef}
            data={filteredItems}
            atBottomStateChange={setAtBottom}
            followOutput={atBottom ? 'smooth' : false}
            itemContent={(_, item) =>
              isTurnSeparator(item) ? (
                <TurnSeparatorRow role={item.role} content={item.content} />
              ) : isHookEvent(item) ? (
                <ActionRow
                  action={{
                    id: item.id,
                    timestamp: item.timestamp,
                    category: 'hook',
                    toolName: item.toolName ?? 'hook',
                    label: item.label,
                    status: 'success',
                  }}
                />
              ) : (
                <ActionRow action={item} />
              )
            }
          />
        )}

        {/* New actions indicator */}
        {showNewIndicator && !atBottom && (
          <button
            onClick={scrollToBottom}
            className="absolute bottom-3 left-1/2 -translate-x-1/2 inline-flex items-center gap-1 px-3 py-1.5 rounded-full bg-indigo-600 text-white text-xs font-medium shadow-lg hover:bg-indigo-500 transition-colors cursor-pointer z-10"
          >
            <ArrowDown className="w-3 h-3" />
            New actions
          </button>
        )}
      </div>
    </div>
  )
}
