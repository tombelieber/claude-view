import {
  Plus, Bug, RefreshCw, FlaskConical,
  FileText, Settings, Server,
  ClipboardList, Lightbulb, Blocks, Tag,
} from 'lucide-react'
import { cn } from '../lib/utils'
import { getCategoryConfig } from '../lib/category-utils'

const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  Plus, Bug, RefreshCw, FlaskConical,
  FileText, Settings, Server,
  ClipboardList, Lightbulb, Blocks, Tag,
}

interface CategoryBadgeProps {
  l1?: string | null
  l2?: string | null
  l3?: string | null
  className?: string
}

/**
 * Renders an AI classification category badge from L1/L2/L3 fields.
 * Displays the L2 category (most useful granularity) with icon.
 */
export function CategoryBadge({ l2, className }: CategoryBadgeProps) {
  if (!l2) return null

  const config = getCategoryConfig(l2)
  const Icon = ICON_MAP[config.icon] || Tag

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-1.5 py-0.5 text-xs font-medium rounded border',
        config.bgColor,
        config.textColor,
        config.borderColor,
        className,
      )}
      title={`AI classified: ${config.label}`}
    >
      <Icon className="w-3 h-3" />
      <span>{config.label}</span>
    </span>
  )
}
