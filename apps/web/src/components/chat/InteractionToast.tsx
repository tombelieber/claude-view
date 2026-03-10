import { Bell } from 'lucide-react'

interface InteractionToastProps {
  visible: boolean
  label: string
  onScrollTo: () => void
}

export function InteractionToast({ visible, label, onScrollTo }: InteractionToastProps) {
  if (!visible) return null

  return (
    <div className="absolute bottom-20 left-1/2 -translate-x-1/2 z-40 animate-in slide-in-from-bottom-2 fade-in-0">
      <button
        type="button"
        onClick={onScrollTo}
        className="inline-flex items-center gap-2 px-4 py-2 bg-amber-500 dark:bg-amber-600 text-white text-xs font-medium rounded-full shadow-lg hover:bg-amber-600 dark:hover:bg-amber-700 transition-colors cursor-pointer"
      >
        <Bell className="w-3.5 h-3.5 animate-bounce" />
        {label}
      </button>
    </div>
  )
}
