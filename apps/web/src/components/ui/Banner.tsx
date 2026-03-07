import { AlertCircle, FlaskConical, Info, X } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'

const VARIANT_CONFIG = {
  error: {
    icon: AlertCircle,
    bar: 'bg-red-50 dark:bg-red-950/40 border-red-200 dark:border-red-800',
    inline: 'bg-red-50 dark:bg-red-950/40 border-red-200 dark:border-red-800',
    iconColor: 'text-red-500 dark:text-red-400',
    textColor: 'text-red-700 dark:text-red-300',
    actionBg:
      'bg-red-100 dark:bg-red-800/50 text-red-800 dark:text-red-200 hover:bg-red-200 dark:hover:bg-red-800',
    dismissColor: 'text-red-400 hover:text-red-600 dark:text-red-500 dark:hover:text-red-300',
  },
  warning: {
    icon: AlertCircle,
    bar: 'bg-amber-50 dark:bg-amber-950/40 border-amber-200 dark:border-amber-800',
    inline: 'bg-amber-50 dark:bg-amber-950/40 border-amber-200 dark:border-amber-800',
    iconColor: 'text-amber-500 dark:text-amber-400',
    textColor: 'text-amber-700 dark:text-amber-300',
    actionBg:
      'bg-amber-100 dark:bg-amber-800/50 text-amber-800 dark:text-amber-200 hover:bg-amber-200 dark:hover:bg-amber-800',
    dismissColor:
      'text-amber-400 hover:text-amber-600 dark:text-amber-500 dark:hover:text-amber-300',
  },
  info: {
    icon: Info,
    bar: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800',
    inline: 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800',
    iconColor: 'text-blue-600 dark:text-blue-400',
    textColor: 'text-blue-800 dark:text-blue-200',
    actionBg:
      'bg-blue-100 dark:bg-blue-800/50 text-blue-800 dark:text-blue-200 hover:bg-blue-200 dark:hover:bg-blue-800',
    dismissColor: 'text-blue-400 hover:text-blue-600 dark:text-blue-500 dark:hover:text-blue-300',
  },
  experimental: {
    icon: FlaskConical,
    bar: 'bg-amber-50/80 dark:bg-amber-950/30 border-amber-200 dark:border-amber-800/60',
    inline: 'bg-amber-50/80 dark:bg-amber-950/30 border-amber-200 dark:border-amber-800/60',
    iconColor: 'text-amber-600 dark:text-amber-400',
    textColor: 'text-amber-800 dark:text-amber-300',
    actionBg:
      'bg-amber-100 dark:bg-amber-800/50 text-amber-800 dark:text-amber-200 hover:bg-amber-200 dark:hover:bg-amber-800',
    dismissColor:
      'text-amber-400 hover:text-amber-600 dark:text-amber-500 dark:hover:text-amber-300',
  },
} as const

export type BannerVariant = keyof typeof VARIANT_CONFIG

export interface BannerAction {
  label: string
  onClick: () => void
  icon?: React.ComponentType<{ className?: string }>
}

export interface BannerProps {
  variant: BannerVariant
  /** 'bar' = full-width top strip (border-b, no radius). 'inline' = rounded card in content flow. */
  layout?: 'bar' | 'inline'
  children: React.ReactNode
  /** If set, banner is dismissible. Value is the localStorage key. */
  dismissKey?: string
  action?: BannerAction
  className?: string
}

export function Banner({
  variant,
  layout = 'inline',
  children,
  dismissKey,
  action,
  className,
}: BannerProps) {
  const [dismissed, setDismissed] = useState(
    () => !!dismissKey && localStorage.getItem(dismissKey) === 'true',
  )

  if (dismissed) return null

  const config = VARIANT_CONFIG[variant]
  const Icon = config.icon

  const handleDismiss = () => {
    if (dismissKey) {
      localStorage.setItem(dismissKey, 'true')
      setDismissed(true)
    }
  }

  const isBar = layout === 'bar'

  return (
    <div
      className={cn(
        'border transition-all ease-out duration-300',
        isBar
          ? cn('border-b border-x-0 border-t-0 px-4 py-2.5', config.bar)
          : cn('rounded-xl p-4', config.inline),
        className,
      )}
      role="alert"
    >
      <div className={cn('flex items-start gap-3', isBar && 'max-w-5xl mx-auto items-center')}>
        <Icon
          className={cn('w-4 h-4 flex-shrink-0', isBar ? 'mt-0' : 'mt-0.5', config.iconColor)}
          aria-hidden="true"
        />

        <div className={cn('flex-1 min-w-0 text-sm', config.textColor)}>{children}</div>

        {action && (
          <button
            type="button"
            onClick={action.onClick}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-lg transition-colors cursor-pointer',
              'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-1',
              config.actionBg,
            )}
          >
            {action.icon && <action.icon className="w-4 h-4" aria-hidden="true" />}
            {action.label}
          </button>
        )}

        {dismissKey && (
          <button
            type="button"
            onClick={handleDismiss}
            className={cn(
              'p-0.5 flex-shrink-0 cursor-pointer transition-colors',
              config.dismissColor,
            )}
            aria-label="Dismiss"
          >
            <X className="w-4 h-4" />
          </button>
        )}
      </div>
    </div>
  )
}
