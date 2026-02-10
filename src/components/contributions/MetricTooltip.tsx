import { HelpCircle } from 'lucide-react'

interface MetricTooltipProps {
  children: React.ReactNode
}

/**
 * CSS-only tooltip triggered by hover on an info icon.
 * No JS state needed — pure CSS group-hover.
 */
export function MetricTooltip({ children }: MetricTooltipProps) {
  return (
    <span className="group relative inline-flex items-center">
      <HelpCircle
        className="w-3.5 h-3.5 text-gray-400 dark:text-gray-500 cursor-help ml-1"
        aria-hidden="true"
      />
      <span
        className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 mb-2 w-64 p-3 text-xs leading-relaxed rounded-lg shadow-lg z-50 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 text-gray-700 dark:text-gray-300 opacity-0 invisible group-hover:opacity-100 group-hover:visible motion-safe:transition-all motion-safe:duration-200 motion-safe:ease-out"
        role="tooltip"
      >
        {children}
      </span>
    </span>
  )
}
