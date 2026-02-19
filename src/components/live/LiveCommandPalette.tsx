import { useState, useMemo, useEffect, useRef, useCallback } from 'react'
import { createPortal } from 'react-dom'
import {
  Search,
  X,
  LayoutGrid,
  List,
  Columns3,
  Monitor,
  Filter,
  ArrowUpDown,
  Trash2,
  Clock,
} from 'lucide-react'
import { cn } from '../../lib/utils'
import { cleanPreviewText } from '../../utils/get-session-title'
import type { LiveViewMode } from './types'
import type { LiveSession } from './use-live-sessions'
import type { LiveSortField } from './live-filter'

type CommandActionType =
  | 'switch-view'
  | 'filter-status'
  | 'sort-by'
  | 'select-session'
  | 'clear-filters'
  | 'toggle-help'

interface CommandItem {
  id: string
  label: string
  description?: string
  icon: React.ComponentType<{ className?: string }> | null
  actionType: CommandActionType
  actionPayload?: any
  keywords: string[]
  shortcut?: string
}

interface LiveCommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  viewMode: LiveViewMode
  onViewModeChange: (mode: LiveViewMode) => void
  sessions: LiveSession[]
  selectedId: string | null
  onSelectSession: (id: string) => void
  onFilterStatus: (statuses: string[]) => void
  onClearFilters: () => void
  onSort: (field: LiveSortField) => void
  onToggleHelp: () => void
}

function fuzzyMatch(query: string, item: CommandItem): number {
  const q = query.toLowerCase()
  const allText = [item.label, item.description ?? '', ...item.keywords]
    .join(' ')
    .toLowerCase()
  if (allText.includes(q)) return 100
  const words = q.split(/\s+/)
  const matchCount = words.filter((w) => allText.includes(w)).length
  return (matchCount / words.length) * 80
}

export function LiveCommandPalette({
  isOpen,
  onClose,
  viewMode,
  onViewModeChange,
  sessions,
  selectedId,
  onSelectSession,
  onFilterStatus,
  onClearFilters,
  onSort,
  onToggleHelp,
}: LiveCommandPaletteProps) {
  const [query, setQuery] = useState('')
  const [highlightedIndex, setHighlightedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const commands = useMemo<CommandItem[]>(() => {
    const items: CommandItem[] = []

    // View modes
    const viewModes: {
      mode: LiveViewMode
      label: string
      icon: React.ComponentType<{ className?: string }>
      shortcut: string
      extraKeywords: string[]
    }[] = [
      {
        mode: 'grid',
        label: 'Switch to Grid view',
        icon: LayoutGrid,
        shortcut: '1',
        extraKeywords: ['grid'],
      },
      {
        mode: 'list',
        label: 'Switch to List view',
        icon: List,
        shortcut: '2',
        extraKeywords: ['list'],
      },
      {
        mode: 'kanban',
        label: 'Switch to Board view',
        icon: Columns3,
        shortcut: '3',
        extraKeywords: ['board', 'kanban'],
      },
      {
        mode: 'monitor',
        label: 'Switch to Monitor view',
        icon: Monitor,
        shortcut: '4',
        extraKeywords: ['monitor'],
      },
    ]

    for (const vm of viewModes) {
      items.push({
        id: `view-${vm.mode}`,
        label: vm.label,
        icon: vm.icon,
        actionType: 'switch-view',
        actionPayload: vm.mode,
        keywords: ['view', 'switch', ...vm.extraKeywords],
        shortcut: vm.shortcut,
      })
    }

    // Session search
    for (const session of sessions) {
      const branchLabel = session.gitBranch ?? 'no branch'
      items.push({
        id: `session-${session.id}`,
        label: `${session.projectDisplayName} — ${branchLabel}`,
        description: cleanPreviewText(session.lastUserMessage, 60),
        icon: null,
        actionType: 'select-session',
        actionPayload: session.id,
        keywords: [
          session.project,
          session.gitBranch ?? '',
          session.id,
        ].filter(Boolean),
      })
    }

    // Filter actions
    const statusFilters: { status: string; label: string }[] = [
      { status: 'needs_you', label: 'Show sessions needing you' },
      { status: 'autonomous', label: 'Show autonomous sessions' },
    ]

    for (const sf of statusFilters) {
      items.push({
        id: `filter-${sf.status}`,
        label: sf.label,
        icon: Filter,
        actionType: 'filter-status',
        actionPayload: sf.status,
        keywords: ['filter', 'show', sf.status],
      })
    }

    items.push({
      id: 'clear-filters',
      label: 'Clear all filters',
      icon: Trash2,
      actionType: 'clear-filters',
      keywords: ['clear', 'reset', 'filter', 'remove'],
    })

    // Sort actions
    const sorts: { field: LiveSortField; label: string }[] = [
      { field: 'last_active', label: 'Sort by last active' },
      { field: 'cost', label: 'Sort by cost' },
      { field: 'turns', label: 'Sort by turns' },
    ]

    for (const s of sorts) {
      items.push({
        id: `sort-${s.field}`,
        label: s.label,
        icon: ArrowUpDown,
        actionType: 'sort-by',
        actionPayload: s.field,
        keywords: ['sort', 'order', s.field],
      })
    }

    // Help
    items.push({
      id: 'toggle-help',
      label: 'Keyboard shortcuts',
      icon: Clock,
      actionType: 'toggle-help',
      keywords: ['help', 'keyboard', 'shortcuts', 'keys'],
    })

    return items
  }, [sessions])

  const filteredItems = useMemo(() => {
    if (!query.trim()) return commands

    return commands
      .map((item) => ({ item, score: fuzzyMatch(query, item) }))
      .filter(({ score }) => score > 0)
      .sort((a, b) => b.score - a.score)
      .map(({ item }) => item)
  }, [query, commands])

  const visibleItems = filteredItems.slice(0, 10)

  // Reset highlight when query changes
  useEffect(() => {
    setHighlightedIndex(0)
  }, [query])

  // Focus input on open, reset state
  useEffect(() => {
    if (isOpen) {
      setQuery('')
      setHighlightedIndex(0)
      // Defer focus to allow portal to mount
      requestAnimationFrame(() => {
        inputRef.current?.focus()
      })
    }
  }, [isOpen])

  const executeAction = useCallback(
    (item: CommandItem) => {
      switch (item.actionType) {
        case 'switch-view':
          onViewModeChange(item.actionPayload as LiveViewMode)
          break
        case 'filter-status':
          onFilterStatus([item.actionPayload as string])
          break
        case 'sort-by':
          onSort(item.actionPayload as LiveSortField)
          break
        case 'select-session':
          onSelectSession(item.actionPayload as string)
          break
        case 'clear-filters':
          onClearFilters()
          break
        case 'toggle-help':
          onToggleHelp()
          break
      }
      onClose()
    },
    [
      onViewModeChange,
      onFilterStatus,
      onSort,
      onSelectSession,
      onClearFilters,
      onToggleHelp,
      onClose,
    ]
  )

  // Keyboard navigation
  useEffect(() => {
    if (!isOpen) return

    function handleKeyDown(e: KeyboardEvent) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setHighlightedIndex((prev) =>
            prev < visibleItems.length - 1 ? prev + 1 : 0
          )
          break
        case 'ArrowUp':
          e.preventDefault()
          setHighlightedIndex((prev) =>
            prev > 0 ? prev - 1 : visibleItems.length - 1
          )
          break
        case 'Enter':
          e.preventDefault()
          if (visibleItems[highlightedIndex]) {
            executeAction(visibleItems[highlightedIndex])
          }
          break
        case 'Escape':
          e.preventDefault()
          onClose()
          break
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, visibleItems, highlightedIndex, executeAction, onClose])

  // Scroll highlighted item into view
  useEffect(() => {
    if (!listRef.current) return
    const highlighted = listRef.current.children[highlightedIndex] as
      | HTMLElement
      | undefined
    highlighted?.scrollIntoView({ block: 'nearest' })
  }, [highlightedIndex])

  if (!isOpen) return null

  return createPortal(
    <div className="fixed inset-0 z-50">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-md"
        onClick={onClose}
      />

      {/* Container */}
      <div className="fixed inset-x-0 top-0 z-50 pt-[12vh] px-4">
        <div className="max-w-lg mx-auto bg-white/90 dark:bg-[#0c0e16]/90 backdrop-blur-xl border border-slate-200/80 dark:border-white/[0.06] rounded-xl shadow-2xl shadow-black/20 dark:shadow-black/50 ring-1 ring-black/5 dark:ring-white/[0.05] overflow-hidden">
          {/* Search input */}
          <div className="flex items-center gap-2 px-3 border-b border-slate-200/80 dark:border-white/[0.06]">
            <Search className="w-4 h-4 text-slate-400 dark:text-slate-500 flex-shrink-0" />
            <input
              ref={inputRef}
              type="text"
              className="flex-1 bg-transparent border-none outline-none text-sm text-slate-900 dark:text-slate-100 placeholder:text-slate-400 dark:placeholder:text-slate-500 py-3"
              placeholder="Type a command or search sessions..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <button
              onClick={onClose}
              className="p-1 rounded hover:bg-slate-100 dark:hover:bg-white/[0.06] text-slate-400 dark:text-slate-500 hover:text-slate-700 dark:hover:text-slate-200 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Results list */}
          <div
            ref={listRef}
            className="max-h-[320px] overflow-y-auto py-1 px-1"
          >
            {visibleItems.length === 0 ? (
              <div className="px-3 py-6 text-center text-sm text-slate-400 dark:text-slate-500">
                No matching commands
              </div>
            ) : (
              visibleItems.map((item, index) => {
                const Icon = item.icon
                return (
                  <div
                    key={item.id}
                    className={cn(
                      'px-3 py-2 flex items-center gap-3 cursor-pointer rounded-md',
                      index === highlightedIndex
                        ? 'bg-emerald-50 dark:bg-emerald-500/[0.08]'
                        : 'hover:bg-slate-50 dark:hover:bg-white/[0.04]'
                    )}
                    onClick={() => executeAction(item)}
                    onMouseEnter={() => setHighlightedIndex(index)}
                  >
                    {Icon ? (
                      <Icon className="w-4 h-4 text-slate-400 dark:text-slate-500 flex-shrink-0" />
                    ) : (
                      <div className="w-4 h-4 flex-shrink-0" />
                    )}
                    <div className="flex-1 min-w-0">
                      <div className="text-sm text-slate-800 dark:text-slate-200">{item.label}</div>
                      {item.description && (
                        <div className="text-xs text-slate-400 dark:text-slate-500 truncate">
                          {item.description}
                        </div>
                      )}
                    </div>
                    {item.shortcut && (
                      <span className="ml-auto text-[10px] font-mono text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-white/[0.06] px-1.5 py-0.5 rounded border border-slate-200 dark:border-white/[0.08]">
                        {item.shortcut}
                      </span>
                    )}
                  </div>
                )
              })
            )}
          </div>

          {/* Footer */}
          <div className="px-3 py-2 border-t border-slate-200/80 dark:border-white/[0.06] text-[10px] text-slate-400 dark:text-slate-500">
            <span className="inline-flex items-center gap-3">
              <span>
                <kbd className="font-mono">↑↓</kbd> Navigate
              </span>
              <span>
                <kbd className="font-mono">Enter</kbd> Select
              </span>
              <span>
                <kbd className="font-mono">Esc</kbd> Close
              </span>
            </span>
          </div>
        </div>
      </div>
    </div>,
    document.body
  )
}
