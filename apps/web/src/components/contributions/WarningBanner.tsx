import { RefreshCw } from 'lucide-react'
import type { ContributionWarning } from '../../types/generated'
import { Banner } from '../ui/Banner'

interface WarningBannerProps {
  warnings: ContributionWarning[]
  onSync?: () => void
  className?: string
}

export function WarningBanner({ warnings, onSync, className }: WarningBannerProps) {
  if (warnings.length === 0) return null

  const hasActionableWarning = warnings.some(
    (w) => w.code === 'GitSyncIncomplete' || w.code === 'PartialData',
  )

  const showSyncAction = onSync && warnings.some((w) => w.code === 'GitSyncIncomplete')

  return (
    <Banner
      variant={hasActionableWarning ? 'warning' : 'info'}
      action={showSyncAction ? { label: 'Sync', onClick: onSync, icon: RefreshCw } : undefined}
      className={className}
    >
      <div className="space-y-1">
        {warnings.map((warning, i) => (
          <p key={i}>{warning.message}</p>
        ))}
      </div>
    </Banner>
  )
}
