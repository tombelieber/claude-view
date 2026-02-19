import { useEffect, useRef, useCallback } from 'react'
import {
  Pin,
  PinOff,
  EyeOff,
  ArrowUpToLine,
  Maximize2,
} from 'lucide-react'
import { cn } from '../../lib/utils'

export interface PaneContextMenuProps {
  x: number
  y: number
  sessionId: string
  isPinned: boolean
  onClose: () => void
  onPin: () => void
  onUnpin: () => void
  onHide: () => void
  onMoveToFront: () => void
  onExpand: () => void
}

interface MenuItem {
  icon: React.ComponentType<{ className?: string }>
  label: string
  action: () => void
}

const MENU_WIDTH = 200
const MENU_ITEM_HEIGHT = 36
const MENU_PADDING = 8 // py-1 top + bottom

export function PaneContextMenu({
  x,
  y,
  isPinned,
  onClose,
  onPin,
  onUnpin,
  onHide,
  onMoveToFront,
  onExpand,
}: PaneContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null)

  // Build menu items based on current state
  const items: MenuItem[] = []

  if (isPinned) {
    items.push({ icon: PinOff, label: 'Unpin pane', action: onUnpin })
  } else {
    items.push({ icon: Pin, label: 'Pin pane', action: onPin })
  }

  items.push({ icon: EyeOff, label: 'Hide pane', action: onHide })
  items.push({ icon: ArrowUpToLine, label: 'Move to front', action: onMoveToFront })
  items.push({ icon: Maximize2, label: 'Expand', action: onExpand })

  // Compute position, flipping if the menu would go off-screen
  const menuHeight = items.length * MENU_ITEM_HEIGHT + MENU_PADDING * 2
  const adjustedX = x + MENU_WIDTH > window.innerWidth ? x - MENU_WIDTH : x
  const adjustedY = y + menuHeight > window.innerHeight ? y - menuHeight : y

  const handleItemClick = useCallback(
    (action: () => void) => {
      action()
      onClose()
    },
    [onClose]
  )

  // Close on click outside, ESC, or scroll
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        onClose()
      }
    }

    function handleScroll() {
      onClose()
    }

    // Use capture phase for click so we intercept before other handlers
    document.addEventListener('mousedown', handleClickOutside, true)
    document.addEventListener('keydown', handleKeyDown, true)
    window.addEventListener('scroll', handleScroll, true)

    return () => {
      document.removeEventListener('mousedown', handleClickOutside, true)
      document.removeEventListener('keydown', handleKeyDown, true)
      window.removeEventListener('scroll', handleScroll, true)
    }
  }, [onClose])

  return (
    <div
      ref={menuRef}
      className={cn(
        'fixed z-50 py-1 min-w-[180px]',
        'bg-gray-800 border border-gray-700 rounded-lg shadow-xl',
        'animate-in fade-in-0 zoom-in-95 duration-100'
      )}
      style={{
        left: Math.max(0, adjustedX),
        top: Math.max(0, adjustedY),
      }}
      role="menu"
      aria-orientation="vertical"
    >
      {items.map((item) => {
        const Icon = item.icon
        return (
          <button
            key={item.label}
            role="menuitem"
            className="flex items-center gap-2 w-full px-3 py-2 text-sm text-gray-300 hover:bg-gray-700 hover:text-white rounded transition-colors"
            onClick={() => handleItemClick(item.action)}
          >
            <Icon className="w-4 h-4 flex-shrink-0" />
            <span>{item.label}</span>
          </button>
        )
      })}
    </div>
  )
}
