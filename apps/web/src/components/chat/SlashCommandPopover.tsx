import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '../../lib/utils'
import { COMMANDS, type SlashCommand, filterCommands } from './commands'

interface SlashCommandPopoverProps {
  input: string
  open: boolean
  onSelect: (cmd: SlashCommand) => void
  onClose: () => void
  commands?: SlashCommand[]
  anchorRef: React.RefObject<HTMLTextAreaElement | null>
}

/**
 * Popover that appears above the textarea when user types "/".
 * Supports keyboard navigation: ArrowUp, ArrowDown, Enter, Escape.
 */
export function SlashCommandPopover({
  input,
  open,
  onSelect,
  onClose,
  commands: commandsProp,
  anchorRef,
}: SlashCommandPopoverProps) {
  const allCommands = commandsProp ?? COMMANDS
  const [activeIndex, setActiveIndex] = useState(0)
  const listRef = useRef<HTMLDivElement>(null)

  // Extract the query portion after "/"
  const slashIndex = input.lastIndexOf('/')
  const query = slashIndex >= 0 ? input.slice(slashIndex + 1) : ''
  const filtered = open ? filterCommands(query).filter((c) => allCommands.includes(c)) : []

  // If using custom commands, filter from those instead
  const results = commandsProp
    ? query
      ? commandsProp.filter(
          (cmd) =>
            cmd.name.toLowerCase().includes(query.toLowerCase()) ||
            cmd.description.toLowerCase().includes(query.toLowerCase()),
        )
      : commandsProp
    : filtered

  // Reset active index when results change
  const resultCount = results.length
  useEffect(() => {
    setActiveIndex(0)
  }, [resultCount])

  // Scroll active item into view
  useEffect(() => {
    if (!listRef.current) return
    const activeEl = listRef.current.children[activeIndex] as HTMLElement | undefined
    activeEl?.scrollIntoView({ block: 'nearest' })
  }, [activeIndex])

  // Keyboard handler attached to the anchor textarea
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!open || results.length === 0) return

      switch (e.key) {
        case 'ArrowDown': {
          e.preventDefault()
          setActiveIndex((prev) => (prev + 1) % results.length)
          break
        }
        case 'ArrowUp': {
          e.preventDefault()
          setActiveIndex((prev) => (prev - 1 + results.length) % results.length)
          break
        }
        case 'Enter': {
          e.preventDefault()
          const selected = results[activeIndex]
          if (selected) onSelect(selected)
          break
        }
        case 'Escape': {
          e.preventDefault()
          onClose()
          break
        }
        case 'Tab': {
          e.preventDefault()
          const selected = results[activeIndex]
          if (selected) onSelect(selected)
          break
        }
      }
    },
    [open, results, activeIndex, onSelect, onClose],
  )

  // Attach/detach keydown listener on the textarea
  useEffect(() => {
    const textarea = anchorRef.current
    if (!textarea || !open) return

    textarea.addEventListener('keydown', handleKeyDown)
    return () => textarea.removeEventListener('keydown', handleKeyDown)
  }, [anchorRef, open, handleKeyDown])

  if (!open || results.length === 0) return null

  const CATEGORY_LABELS: Record<string, string> = {
    mode: 'Mode',
    session: 'Session',
    action: 'Action',
    info: 'Info',
  }

  // Group by category for display
  const grouped: { category: string; items: { cmd: SlashCommand; globalIdx: number }[] }[] = []
  let globalIdx = 0
  const seen = new Set<string>()
  for (const cmd of results) {
    if (!seen.has(cmd.category)) {
      seen.add(cmd.category)
      grouped.push({ category: cmd.category, items: [] })
    }
    const group = grouped.find((g) => g.category === cmd.category)
    group?.items.push({ cmd, globalIdx })
    globalIdx++
  }

  return (
    <div
      className="absolute bottom-full left-0 right-0 mb-1 max-h-64 overflow-y-auto rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg z-50"
      role="listbox"
      aria-label="Slash commands"
      ref={listRef}
    >
      {grouped.map((group) => (
        <div key={group.category}>
          <div className="sticky top-0 px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500 bg-gray-50 dark:bg-gray-800/50">
            {CATEGORY_LABELS[group.category] ?? group.category}
          </div>
          {group.items.map(({ cmd, globalIdx: idx }) => (
            <button
              key={cmd.name}
              type="button"
              role="option"
              aria-selected={idx === activeIndex}
              onMouseEnter={() => setActiveIndex(idx)}
              onClick={() => onSelect(cmd)}
              className={cn(
                'flex items-center gap-3 w-full px-3 py-2 text-left cursor-pointer transition-colors',
                idx === activeIndex
                  ? 'bg-blue-50 dark:bg-blue-950/30'
                  : 'hover:bg-gray-50 dark:hover:bg-gray-800',
              )}
            >
              <span className="text-sm font-medium text-gray-900 dark:text-gray-100 min-w-[60px]">
                /{cmd.name}
              </span>
              <span className="text-xs text-gray-500 dark:text-gray-400 truncate">
                {cmd.description}
              </span>
            </button>
          ))}
        </div>
      ))}
    </div>
  )
}
