import { ArrowLeft, Check, ChevronRight, ExternalLink } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '../../lib/utils'
import type { PaletteItem, PaletteSection } from './palette-items'

interface ChatPaletteProps {
  sections: PaletteSection[]
  filter: string
  onClose: () => void
}

// ---------------------------------------------------------------------------
// Filter logic
// ---------------------------------------------------------------------------

function matchesFilter(item: PaletteItem, query: string): boolean {
  if (!query) return true
  const q = query.toLowerCase()
  if (item.type === 'command') {
    return item.name.toLowerCase().includes(q) || item.description.toLowerCase().includes(q)
  }
  if ('label' in item) {
    return item.label.toLowerCase().includes(q)
  }
  return false
}

// ---------------------------------------------------------------------------
// Flatten sections into a selectable item list for arrow navigation
// ---------------------------------------------------------------------------

interface FlatItem {
  item: PaletteItem
  sectionLabel: string
  disabled: boolean
}

function flattenSections(sections: PaletteSection[]): FlatItem[] {
  const flat: FlatItem[] = []
  for (const section of sections) {
    for (const item of section.items) {
      const disabled = item.type === 'action' && !!item.disabled
      flat.push({ item, sectionLabel: section.label, disabled })
    }
  }
  return flat
}

// ---------------------------------------------------------------------------
// Item renderers
// ---------------------------------------------------------------------------

function ActionItemRow({
  item,
  onClose,
  active,
}: {
  item: Extract<PaletteItem, { type: 'action' }>
  onClose: () => void
  active: boolean
}) {
  return (
    <button
      type="button"
      disabled={item.disabled}
      onClick={() => {
        if (!item.disabled) {
          item.onSelect()
          onClose()
        }
      }}
      className={cn(
        'flex items-center gap-3 w-full px-3 py-2 text-left transition-colors',
        item.disabled
          ? 'opacity-50 cursor-not-allowed'
          : 'cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800',
        active && !item.disabled && 'bg-blue-50 dark:bg-blue-950/30',
      )}
    >
      <item.icon className="w-4 h-4 text-gray-400 flex-shrink-0" />
      <span className="text-sm text-gray-900 dark:text-gray-100">{item.label}</span>
      {item.hint && (
        <span className="ml-auto text-xs text-gray-400 dark:text-gray-500">{item.hint}</span>
      )}
    </button>
  )
}

function SubmenuItemRow({
  item,
  onOpen,
  active,
}: {
  item: Extract<PaletteItem, { type: 'submenu' }>
  onOpen: () => void
  active: boolean
}) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className={cn(
        'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors',
        active && 'bg-blue-50 dark:bg-blue-950/30',
      )}
    >
      <item.icon className="w-4 h-4 text-gray-400 flex-shrink-0" />
      <span className="text-sm text-gray-900 dark:text-gray-100">{item.label}</span>
      <span className="ml-auto flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
        {item.current}
        <ChevronRight className="w-3 h-3" />
      </span>
    </button>
  )
}

function LinkItemRow({
  item,
  active,
}: {
  item: Extract<PaletteItem, { type: 'link' }>
  active: boolean
}) {
  return (
    <a
      href={item.href}
      className={cn(
        'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors',
        active && 'bg-blue-50 dark:bg-blue-950/30',
      )}
    >
      <item.icon className="w-4 h-4 text-gray-400 flex-shrink-0" />
      <span className="text-sm text-gray-900 dark:text-gray-100">{item.label}</span>
      <span className="ml-auto flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500">
        {item.badge && <span>{item.badge}</span>}
        <ExternalLink className="w-3 h-3" />
      </span>
    </a>
  )
}

function CommandItemRow({
  item,
  onClose,
  active,
}: {
  item: Extract<PaletteItem, { type: 'command' }>
  onClose: () => void
  active: boolean
}) {
  return (
    <button
      type="button"
      onClick={() => {
        item.onSelect()
        onClose()
      }}
      className={cn(
        'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors',
        active && 'bg-blue-50 dark:bg-blue-950/30',
      )}
    >
      <span className="text-sm font-medium text-gray-900 dark:text-gray-100 min-w-[60px]">
        /{item.name}
      </span>
      {item.description && (
        <span className="text-xs text-gray-500 dark:text-gray-400 truncate">
          {item.description}
        </span>
      )}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Submenu view
// ---------------------------------------------------------------------------

function SubmenuView({
  item,
  onBack,
  onClose,
}: {
  item: Extract<PaletteItem, { type: 'submenu' }>
  onBack: () => void
  onClose: () => void
}) {
  const [activeIdx, setActiveIdx] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // Reset index when items change
  useEffect(() => {
    setActiveIdx(0)
  }, [item.items.length])

  // Scroll active into view
  useEffect(() => {
    const container = listRef.current
    if (!container) return
    requestAnimationFrame(() => {
      const el = container.querySelector('[data-active="true"]') as HTMLElement | null
      if (!el) return
      if (el.offsetTop < container.clientHeight / 2) {
        container.scrollTop = 0
      } else {
        el.scrollIntoView({ block: 'nearest' })
      }
    })
  }, [activeIdx])

  // Keyboard nav inside submenu
  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setActiveIdx((prev) => (prev + 1) % item.items.length)
          break
        case 'ArrowUp':
          e.preventDefault()
          setActiveIdx((prev) => (prev - 1 + item.items.length) % item.items.length)
          break
        case 'Enter': {
          e.preventDefault()
          const sub = item.items[activeIdx]
          if (sub) {
            item.onSelect(sub.id)
            onClose()
          }
          break
        }
        case 'Escape':
          e.preventDefault()
          onBack()
          break
      }
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [item, activeIdx, onBack, onClose])

  return (
    <div ref={listRef}>
      <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-100 dark:border-gray-800">
        <button
          type="button"
          onClick={onBack}
          aria-label="back"
          className="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
        >
          <ArrowLeft className="w-4 h-4 text-gray-500" />
        </button>
        <span className="text-sm font-medium text-gray-900 dark:text-gray-100">{item.label}</span>
      </div>
      {item.items.map((sub, idx) => (
        <button
          key={sub.id}
          type="button"
          data-active={idx === activeIdx}
          onMouseEnter={() => setActiveIdx(idx)}
          onClick={() => {
            item.onSelect(sub.id)
            onClose()
          }}
          className={cn(
            'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer transition-colors',
            sub.active
              ? 'bg-blue-50 dark:bg-blue-950/30 text-blue-700 dark:text-blue-300'
              : idx === activeIdx
                ? 'bg-gray-100 dark:bg-gray-800 text-gray-900 dark:text-gray-100'
                : 'hover:bg-gray-50 dark:hover:bg-gray-800 text-gray-900 dark:text-gray-100',
          )}
        >
          {sub.active && <Check className="w-4 h-4 flex-shrink-0" />}
          {!sub.active && <span className="w-4" />}
          <span className="text-sm">{sub.label}</span>
        </button>
      ))}
      {item.warning && (
        <div className="px-3 py-2 text-xs text-amber-600 dark:text-amber-400 border-t border-gray-100 dark:border-gray-800">
          {item.warning}
        </div>
      )}
    </div>
  )
}

// ---------------------------------------------------------------------------
// Main component
// ---------------------------------------------------------------------------

export function ChatPalette({ sections, filter, onClose }: ChatPaletteProps) {
  const [activeSubmenu, setActiveSubmenu] = useState<{
    sectionLabel: string
    itemLabel: string
  } | null>(null)
  const [activeIndex, setActiveIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // Filter sections
  const filteredSections = sections
    .map((section) => ({
      ...section,
      items: section.items.filter((item) => matchesFilter(item, filter)),
    }))
    .filter((section) => section.items.length > 0)

  // Flatten for arrow navigation
  const flatItems = flattenSections(filteredSections)

  // Reset active index when filter/items change
  const itemCount = flatItems.length
  useEffect(() => {
    setActiveIndex(0)
  }, [itemCount, filter])

  // Ensure scroll starts at top on mount (positioned bottom-full, browsers can default to bottom)
  useEffect(() => {
    const container = listRef.current
    if (container) {
      container.scrollTop = 0
    }
  }, [])

  // Scroll active item into view
  useEffect(() => {
    const container = listRef.current
    if (!container) return
    // Use rAF to ensure DOM is committed before scrolling
    requestAnimationFrame(() => {
      const el = container.querySelector('[data-active="true"]') as HTMLElement | null
      if (!el) return
      // If the active element is at or near the top of the list, snap scroll to 0
      if (el.offsetTop < container.clientHeight / 2) {
        container.scrollTop = 0
      } else {
        el.scrollIntoView({ block: 'nearest' })
      }
    })
  }, [activeIndex])

  // Find next non-disabled index
  const findNextIndex = useCallback(
    (current: number, direction: 1 | -1): number => {
      if (flatItems.length === 0) return 0
      let next = (current + direction + flatItems.length) % flatItems.length
      let tries = 0
      while (flatItems[next]?.disabled && tries < flatItems.length) {
        next = (next + direction + flatItems.length) % flatItems.length
        tries++
      }
      return next
    },
    [flatItems],
  )

  // Activate the item at the current index
  const activateItem = useCallback(
    (idx: number) => {
      const entry = flatItems[idx]
      if (!entry || entry.disabled) return

      const { item, sectionLabel } = entry
      if (item.type === 'action') {
        item.onSelect()
        onClose()
      } else if (item.type === 'submenu') {
        setActiveSubmenu({ sectionLabel, itemLabel: item.label })
      } else if (item.type === 'link') {
        window.location.href = item.href
      } else if (item.type === 'command') {
        item.onSelect()
        onClose()
      }
    },
    [flatItems, onClose],
  )

  // Keyboard handler
  useEffect(() => {
    if (activeSubmenu) return // submenu handles its own keys

    function handleKey(e: KeyboardEvent) {
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault()
          setActiveIndex((prev) => findNextIndex(prev, 1))
          break
        case 'ArrowUp':
          e.preventDefault()
          setActiveIndex((prev) => findNextIndex(prev, -1))
          break
        case 'Enter':
        case 'Tab':
          e.preventDefault()
          activateItem(activeIndex)
          break
        case 'Escape':
          e.preventDefault()
          onClose()
          break
      }
    }
    document.addEventListener('keydown', handleKey)
    return () => document.removeEventListener('keydown', handleKey)
  }, [activeSubmenu, activeIndex, findNextIndex, activateItem, onClose])

  // Find active submenu item
  if (activeSubmenu) {
    const section = sections.find((s) => s.label === activeSubmenu.sectionLabel)
    const item = section?.items.find(
      (i) => i.type === 'submenu' && i.label === activeSubmenu.itemLabel,
    )
    if (item && item.type === 'submenu') {
      return (
        <div
          data-testid="command-palette"
          className="max-h-72 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg"
        >
          <SubmenuView item={item} onBack={() => setActiveSubmenu(null)} onClose={onClose} />
        </div>
      )
    }
  }

  if (filteredSections.length === 0) return null

  // Track flat index across sections for rendering
  let flatIdx = 0

  return (
    <div
      ref={listRef}
      data-testid="command-palette"
      className="max-h-72 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg"
    >
      {filteredSections.map((section) => (
        <div key={section.label}>
          <div className="sticky top-0 px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-800/50">
            {section.label}
          </div>
          {section.items.map((item) => {
            const idx = flatIdx++
            const isActive = idx === activeIndex
            if (item.type === 'action') {
              return (
                <div
                  key={item.label}
                  data-active={isActive}
                  onMouseEnter={() => setActiveIndex(idx)}
                >
                  <ActionItemRow item={item} onClose={onClose} active={isActive} />
                </div>
              )
            }
            if (item.type === 'submenu') {
              return (
                <div
                  key={item.label}
                  data-active={isActive}
                  onMouseEnter={() => setActiveIndex(idx)}
                >
                  <SubmenuItemRow
                    item={item}
                    onOpen={() =>
                      setActiveSubmenu({ sectionLabel: section.label, itemLabel: item.label })
                    }
                    active={isActive}
                  />
                </div>
              )
            }
            if (item.type === 'link') {
              return (
                <div
                  key={item.label}
                  data-active={isActive}
                  onMouseEnter={() => setActiveIndex(idx)}
                >
                  <LinkItemRow item={item} active={isActive} />
                </div>
              )
            }
            if (item.type === 'command') {
              return (
                <div
                  key={item.name}
                  data-active={isActive}
                  onMouseEnter={() => setActiveIndex(idx)}
                >
                  <CommandItemRow item={item} onClose={onClose} active={isActive} />
                </div>
              )
            }
            return null
          })}
        </div>
      ))}
    </div>
  )
}
