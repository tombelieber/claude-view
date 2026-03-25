import type { ReactNode } from 'react'

export type CardVariant = 'question' | 'permission' | 'plan' | 'elicitation'

interface ResolvedState {
  label: string
  variant: 'success' | 'denied' | 'neutral'
}

export interface InteractiveCardShellProps {
  variant: CardVariant
  header: string
  icon?: ReactNode
  resolved?: ResolvedState
  children: ReactNode
  actions?: ReactNode
}

const VARIANT_COLORS: Record<
  CardVariant,
  { border: string; bg: string; headerBg: string; text: string }
> = {
  question: {
    border: 'border-amber-200/50 dark:border-amber-500/20',
    bg: 'bg-amber-50/30 dark:bg-amber-900/10',
    headerBg: 'bg-amber-100/50 dark:bg-amber-900/20',
    text: 'text-amber-600 dark:text-amber-400',
  },
  permission: {
    border: 'border-amber-200/50 dark:border-amber-500/20',
    bg: 'bg-amber-50/30 dark:bg-amber-900/10',
    headerBg: 'bg-amber-100/50 dark:bg-amber-900/20',
    text: 'text-amber-600 dark:text-amber-400',
  },
  plan: {
    border: 'border-blue-200/50 dark:border-blue-500/20',
    bg: 'bg-blue-50/30 dark:bg-blue-900/10',
    headerBg: 'bg-blue-100/50 dark:bg-blue-900/20',
    text: 'text-blue-600 dark:text-blue-400',
  },
  elicitation: {
    border: 'border-gray-200/50 dark:border-gray-700/50',
    bg: 'bg-gray-50/30 dark:bg-gray-800/20',
    headerBg: 'bg-gray-100/50 dark:bg-gray-800/40',
    text: 'text-gray-600 dark:text-gray-400',
  },
}

const RESOLVED_BADGE_COLORS: Record<ResolvedState['variant'], string> = {
  success: 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400',
  denied: 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400',
  neutral: 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400',
}

export function InteractiveCardShell({
  variant,
  header,
  icon,
  resolved,
  children,
  actions,
}: InteractiveCardShellProps) {
  const colors = VARIANT_COLORS[variant]
  const isResolved = !!resolved

  return (
    <div
      className={`rounded-lg border ${colors.border} ${colors.bg} overflow-hidden ${
        isResolved ? 'opacity-60 pointer-events-none' : ''
      }`}
    >
      {/* Header */}
      <div className={`px-3 py-2 ${colors.headerBg} flex items-center justify-between`}>
        <div className="flex items-center gap-2 min-w-0">
          {icon && <span className={`flex-shrink-0 ${colors.text}`}>{icon}</span>}
          <span className={`text-xs font-medium ${colors.text} truncate`}>{header}</span>
        </div>
        {resolved && (
          <span
            className={`text-xs font-medium px-2 py-0.5 rounded-full ${RESOLVED_BADGE_COLORS[resolved.variant]}`}
          >
            {resolved.label}
          </span>
        )}
      </div>

      {/* Content */}
      <div className="px-3 py-2">{children}</div>

      {/* Actions — hidden when resolved */}
      {!isResolved && actions && (
        <div className="px-3 py-2 border-t border-gray-200/30 dark:border-gray-700/30 flex items-center justify-end gap-2">
          {actions}
        </div>
      )}
    </div>
  )
}
