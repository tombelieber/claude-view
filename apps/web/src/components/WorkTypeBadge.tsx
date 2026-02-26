import { Zap, ClipboardList, Bug, Briefcase, Sparkles } from 'lucide-react'
import { cn } from '../lib/utils'
import { getWorkTypeConfig, type WorkType } from '../lib/work-type-utils'

// Re-export for convenience
export type { WorkType } from '../lib/work-type-utils'

export interface WorkTypeBadgeProps {
  /** Work type classification */
  workType: WorkType | string | null | undefined
  /** Optional className for additional styling */
  className?: string
  /** Show label text (default: true) */
  showLabel?: boolean
}

/** Icon mapping for each work type */
const WORK_TYPE_ICONS: Record<string, React.ComponentType<{ className?: string }>> = {
  deep_work: Briefcase,
  quick_ask: Zap,
  planning: ClipboardList,
  bug_fix: Bug,
  standard: Sparkles,
}

/**
 * WorkTypeBadge displays the work type classification for a session.
 *
 * Work types are rule-based classifications:
 * - Deep Work: duration > 30min, files_edited > 5, LOC > 200
 * - Quick Ask: duration < 5min, turn_count < 3, no edits
 * - Planning: skills contain "brainstorming" or "plan", low edits
 * - Bug Fix: skills contain "debugging", moderate edits
 * - Standard: Everything else
 */
export function WorkTypeBadge({ workType, className, showLabel = true }: WorkTypeBadgeProps) {
  // Don't render if no work type (data not yet computed)
  if (!workType) {
    return null
  }

  const config = getWorkTypeConfig(workType)
  const Icon = WORK_TYPE_ICONS[workType] || Sparkles

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded border',
        config.bgColor,
        config.textColor,
        config.borderColor,
        className
      )}
      title={config.title}
    >
      <Icon className="w-3 h-3" />
      {showLabel && <span>{config.label}</span>}
    </span>
  )
}
