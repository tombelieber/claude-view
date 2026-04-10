import { ChevronDown, ChevronRight } from 'lucide-react'

interface SectionHeaderProps {
  title: string
  collapsed: boolean
  onToggle: () => void
  actions?: React.ReactNode
}

export function SectionHeader({ title, collapsed, onToggle, actions }: SectionHeaderProps) {
  return (
    <div className="flex items-center gap-1 w-full px-3 py-1.5 text-xs font-semibold uppercase tracking-wider text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800 select-none">
      <button
        type="button"
        onClick={onToggle}
        className="flex min-w-0 flex-1 items-center gap-1 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-sm"
      >
        {collapsed ? (
          <ChevronRight className="w-3.5 h-3.5 shrink-0" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 shrink-0" />
        )}
        <span className="truncate">{title}</span>
      </button>
      {actions && <span className="ml-auto flex shrink-0 items-center gap-1">{actions}</span>}
    </div>
  )
}
