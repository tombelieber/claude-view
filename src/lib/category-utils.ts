/**
 * AI classification category utilities.
 *
 * Categories come from Claude Haiku classification (L1/L2/L3).
 * Separate from WorkType (rule-based, Theme 3).
 */

export interface CategoryConfig {
  label: string
  bgColor: string
  textColor: string
  borderColor: string
  icon: string // lucide icon name
}

/**
 * L2 category display config.
 * L2 is the most useful granularity for badges.
 */
/** Neutral style shared by all categories — icon + text label differentiate. */
const NEUTRAL = {
  bgColor: 'bg-gray-50 dark:bg-gray-800',
  textColor: 'text-gray-600 dark:text-gray-400',
  borderColor: 'border-gray-200 dark:border-gray-700',
} as const

export const CATEGORY_L2_CONFIG: Record<string, CategoryConfig> = {
  // code_work children
  feature:      { label: 'Feature',      ...NEUTRAL, icon: 'Plus' },
  bugfix:       { label: 'Bug Fix',      ...NEUTRAL, icon: 'Bug' },
  refactor:     { label: 'Refactor',     ...NEUTRAL, icon: 'RefreshCw' },
  testing:      { label: 'Testing',      ...NEUTRAL, icon: 'FlaskConical' },
  // support_work children
  docs:         { label: 'Docs',         ...NEUTRAL, icon: 'FileText' },
  config:       { label: 'Config',       ...NEUTRAL, icon: 'Settings' },
  ops:          { label: 'Ops',          ...NEUTRAL, icon: 'Server' },
  // thinking_work children
  planning:     { label: 'Planning',     ...NEUTRAL, icon: 'ClipboardList' },
  explanation:  { label: 'Learning',     ...NEUTRAL, icon: 'Lightbulb' },
  architecture: { label: 'Architecture', ...NEUTRAL, icon: 'Blocks' },
}

const DEFAULT_CONFIG: CategoryConfig = {
  label: 'Other',
  bgColor: 'bg-gray-50 dark:bg-gray-800',
  textColor: 'text-gray-600 dark:text-gray-400',
  borderColor: 'border-gray-200 dark:border-gray-700',
  icon: 'Tag',
}

export function getCategoryConfig(l2: string | null | undefined): CategoryConfig {
  if (!l2) return DEFAULT_CONFIG
  return CATEGORY_L2_CONFIG[l2] || DEFAULT_CONFIG
}
