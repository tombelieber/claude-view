import { ArrowLeft, Check, ChevronRight, ExternalLink } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
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
// Item renderers
// ---------------------------------------------------------------------------

function ActionItemRow({
  item,
  onClose,
}: {
  item: Extract<PaletteItem, { type: 'action' }>
  onClose: () => void
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
}: {
  item: Extract<PaletteItem, { type: 'submenu' }>
  onOpen: () => void
}) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
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

function LinkItemRow({ item }: { item: Extract<PaletteItem, { type: 'link' }> }) {
  return (
    <a
      href={item.href}
      className="flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
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
}: {
  item: Extract<PaletteItem, { type: 'command' }>
  onClose: () => void
}) {
  return (
    <button
      type="button"
      onClick={() => {
        item.onSelect()
        onClose()
      }}
      className="flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
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
  return (
    <div>
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
      {item.items.map((sub) => (
        <button
          key={sub.id}
          type="button"
          onClick={() => {
            item.onSelect(sub.id)
            onClose()
          }}
          className={cn(
            'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer transition-colors',
            sub.active
              ? 'bg-blue-50 dark:bg-blue-950/30 text-blue-700 dark:text-blue-300'
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

  // Escape key handler
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        if (activeSubmenu) {
          setActiveSubmenu(null)
        } else {
          onClose()
        }
      }
    },
    [activeSubmenu, onClose],
  )

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [handleKeyDown])

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

  // Filter sections
  const filteredSections = sections
    .map((section) => ({
      ...section,
      items: section.items.filter((item) => matchesFilter(item, filter)),
    }))
    .filter((section) => section.items.length > 0)

  if (filteredSections.length === 0) return null

  return (
    <div
      data-testid="command-palette"
      className="max-h-72 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg"
    >
      {filteredSections.map((section) => (
        <div key={section.label}>
          <div className="sticky top-0 px-3 py-1.5 text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-800/50">
            {section.label}
          </div>
          {section.items.map((item) => {
            if (item.type === 'action') {
              return <ActionItemRow key={item.label} item={item} onClose={onClose} />
            }
            if (item.type === 'submenu') {
              return (
                <SubmenuItemRow
                  key={item.label}
                  item={item}
                  onOpen={() =>
                    setActiveSubmenu({ sectionLabel: section.label, itemLabel: item.label })
                  }
                />
              )
            }
            if (item.type === 'link') {
              return <LinkItemRow key={item.label} item={item} />
            }
            if (item.type === 'command') {
              return <CommandItemRow key={item.name} item={item} onClose={onClose} />
            }
            return null
          })}
        </div>
      ))}
    </div>
  )
}
