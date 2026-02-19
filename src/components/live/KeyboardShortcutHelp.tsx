import { useEffect } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'

interface KeyboardShortcutHelpProps {
  isOpen: boolean
  onClose: () => void
}

interface ShortcutEntry {
  keys: string[]
  description: string
}

const navigationShortcuts: ShortcutEntry[] = [
  { keys: ['j', '/', 'k'], description: 'Next / Previous session' },
  { keys: ['Enter'], description: 'Open selected session' },
  { keys: ['Esc'], description: 'Deselect' },
]

const viewShortcuts: ShortcutEntry[] = [
  { keys: ['1'], description: 'Board view' },
  { keys: ['2'], description: 'Grid view' },
  { keys: ['3'], description: 'List view' },
  { keys: ['4'], description: 'Monitor view' },
  { keys: ['g', 'g'], description: 'Go to Grid' },
  { keys: ['g', 'l'], description: 'Go to List' },
  { keys: ['g', 'k'], description: 'Go to Board' },
  { keys: ['g', 'm'], description: 'Go to Monitor' },
]

const actionShortcuts: ShortcutEntry[] = [
  { keys: ['/'], description: 'Search' },
  { keys: ['?'], description: 'This help' },
]

function Kbd({ children }: { children: string }) {
  return (
    <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded text-[11px] font-mono text-gray-700 dark:text-gray-300">
      {children}
    </kbd>
  )
}

function ShortcutRow({ entry }: { entry: ShortcutEntry }) {
  const isSeparated = entry.keys.length === 3 && entry.keys[1] === '/'

  return (
    <div className="flex items-center justify-between py-1.5">
      <div className="flex items-center gap-1">
        {isSeparated ? (
          <>
            <Kbd>{entry.keys[0]}</Kbd>
            <span className="text-gray-400 dark:text-gray-500 text-xs">/</span>
            <Kbd>{entry.keys[2]}</Kbd>
          </>
        ) : (
          entry.keys.map((key, i) => (
            <Kbd key={i}>{key}</Kbd>
          ))
        )}
      </div>
      <span className="text-sm text-gray-500 dark:text-gray-400">{entry.description}</span>
    </div>
  )
}

function ShortcutSection({ title, shortcuts }: { title: string; shortcuts: ShortcutEntry[] }) {
  return (
    <div>
      <h3 className="text-xs font-semibold text-gray-400 dark:text-gray-500 uppercase tracking-wider mb-2">
        {title}
      </h3>
      <div className="space-y-0.5">
        {shortcuts.map((entry, i) => (
          <ShortcutRow key={i} entry={entry} />
        ))}
      </div>
    </div>
  )
}

export function KeyboardShortcutHelp({ isOpen, onClose }: KeyboardShortcutHelpProps) {
  useEffect(() => {
    if (!isOpen) return

    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
        e.preventDefault()
        e.stopPropagation()
      }
    }

    document.addEventListener('keydown', handler, true)
    return () => document.removeEventListener('keydown', handler, true)
  }, [isOpen, onClose])

  if (!isOpen) return null

  return createPortal(
    <div
      className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex justify-center"
      onClick={onClose}
    >
      <div
        className="bg-white dark:bg-gray-900 border border-gray-300 dark:border-gray-700 rounded-xl shadow-2xl max-w-md w-full mx-auto mt-[15vh] p-6 h-fit"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Keyboard Shortcuts</h2>
          <button
            onClick={onClose}
            className="p-1 rounded-lg text-gray-500 dark:text-gray-400 hover:text-gray-800 dark:hover:text-gray-200 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="space-y-5">
          <ShortcutSection title="Navigation" shortcuts={navigationShortcuts} />
          <ShortcutSection title="Views" shortcuts={viewShortcuts} />
          <ShortcutSection title="Actions" shortcuts={actionShortcuts} />
        </div>
      </div>
    </div>,
    document.body
  )
}
