import { WifiOff } from 'lucide-react'
import { useState } from 'react'
import { cn } from '../../lib/utils'

interface PluginHealthPanelProps {
  orphanCount: number
  conflictCount: number
  unusedCount: number
  cliError: string | null
  onShowOrphaned?: () => void
  onShowConflicts?: () => void
  onShowUnused?: () => void
}

export function PluginHealthPanel({
  orphanCount,
  conflictCount,
  unusedCount,
  cliError,
  onShowOrphaned,
  onShowConflicts,
  onShowUnused,
}: PluginHealthPanelProps) {
  const [open, setOpen] = useState(true)

  if (cliError) {
    return (
      <div className="mx-7 mt-4 rounded-xl border border-apple-sep2 bg-white px-4 py-3 flex items-center gap-2 shadow-[0_1px_4px_rgba(0,0,0,0.04)]">
        <WifiOff className="w-4 h-4 text-apple-red flex-shrink-0" />
        <span className="text-xs text-apple-text2">CLI unavailable: {cliError}</span>
      </div>
    )
  }

  if (orphanCount === 0 && conflictCount === 0 && unusedCount === 0) return null

  const issueCount = [orphanCount > 0, conflictCount > 0, unusedCount > 0].filter(Boolean).length

  return (
    <div className="mx-7 mt-4 rounded-xl border border-apple-sep2 bg-white overflow-hidden shadow-[0_1px_4px_rgba(0,0,0,0.04)]">
      {/* Header */}
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        className="w-full flex items-center justify-between px-4 py-2.5 text-left"
      >
        <div className="flex items-center gap-1.5">
          {orphanCount > 0 && (
            <span className="w-[7px] h-[7px] rounded-full bg-apple-red inline-block" />
          )}
          {conflictCount > 0 && (
            <span
              className={cn(
                'w-[7px] h-[7px] rounded-full bg-apple-orange inline-block',
                orphanCount > 0 && '-ml-0.5',
              )}
            />
          )}
          <span className="text-xs font-semibold text-apple-text1">Plugin health</span>
          <span className="text-xs text-apple-text3 ml-1">
            — {issueCount} issue{issueCount !== 1 ? 's' : ''}
          </span>
        </div>
        <span className="text-xs text-apple-text3">{open ? '▾' : '▸'}</span>
      </button>

      {/* Body */}
      {open && (
        <div className="border-t border-apple-sep2">
          {orphanCount > 0 && (
            <HealthRow
              color="bg-apple-red"
              label={
                <>
                  <strong>{orphanCount} orphaned</strong> — source path missing, can't update or
                  verify
                </>
              }
              onShow={onShowOrphaned}
            />
          )}
          {conflictCount > 0 && (
            <HealthRow
              color="bg-apple-orange"
              label={
                <>
                  <strong>{conflictCount} conflicts</strong> — installed from multiple sources;
                  locally installed version wins
                </>
              }
              onShow={onShowConflicts}
            />
          )}
          {unusedCount > 0 && (
            <HealthRow
              color="bg-apple-text3"
              label={
                <>
                  <strong>{unusedCount} unused</strong> — not invoked in 30 days
                </>
              }
              onShow={onShowUnused}
            />
          )}
        </div>
      )}
    </div>
  )
}

function HealthRow({
  color,
  label,
  onShow,
}: {
  color: string
  label: React.ReactNode
  onShow?: () => void
}) {
  const classes = cn(
    'w-full flex items-center justify-between gap-3 px-4 py-2.5 text-left',
    'border-b border-apple-sep2 last:border-b-0',
    'hover:bg-apple-bg transition-colors',
    onShow && 'cursor-pointer',
  )
  const content = (
    <>
      <div className="flex items-center gap-2.5 flex-1 min-w-0">
        <span className={cn('w-1.5 h-1.5 rounded-full flex-shrink-0', color)} />
        <span className="text-xs text-apple-text2">{label}</span>
      </div>
      {onShow && (
        <span className="text-xs text-apple-blue font-medium whitespace-nowrap flex-shrink-0">
          Show →
        </span>
      )}
    </>
  )
  if (onShow) {
    return (
      <button type="button" onClick={onShow} className={classes}>
        {content}
      </button>
    )
  }
  return <div className={classes}>{content}</div>
}
