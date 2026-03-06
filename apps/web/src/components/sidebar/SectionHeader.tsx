import { ChevronDown, ChevronRight } from 'lucide-react'

interface SectionHeaderProps {
  title: string
  collapsed: boolean
  onToggle: () => void
  actions?: React.ReactNode
}

export function SectionHeader({ title, collapsed, onToggle, actions }: SectionHeaderProps) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className="flex items-center gap-1 w-full px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 select-none"
    >
      {collapsed ? (
        <ChevronRight className="w-3.5 h-3.5 shrink-0" />
      ) : (
        <ChevronDown className="w-3.5 h-3.5 shrink-0" />
      )}
      <span className="truncate">{title}</span>
      {actions && (
        <span
          className="ml-auto flex items-center gap-1"
          onClick={(e) => e.stopPropagation()}
          onKeyDown={(e) => e.stopPropagation()}
        >
          {actions}
        </span>
      )}
    </button>
  )
}
