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
export const CATEGORY_L2_CONFIG: Record<string, CategoryConfig> = {
  // code_work children
  feature: {
    label: 'Feature',
    bgColor: 'bg-blue-50 dark:bg-blue-950/30',
    textColor: 'text-blue-700 dark:text-blue-400',
    borderColor: 'border-blue-200 dark:border-blue-800',
    icon: 'Plus',
  },
  bugfix: {
    label: 'Bug Fix',
    bgColor: 'bg-red-50 dark:bg-red-950/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    icon: 'Bug',
  },
  refactor: {
    label: 'Refactor',
    bgColor: 'bg-orange-50 dark:bg-orange-950/30',
    textColor: 'text-orange-700 dark:text-orange-400',
    borderColor: 'border-orange-200 dark:border-orange-800',
    icon: 'RefreshCw',
  },
  testing: {
    label: 'Testing',
    bgColor: 'bg-green-50 dark:bg-green-950/30',
    textColor: 'text-green-700 dark:text-green-400',
    borderColor: 'border-green-200 dark:border-green-800',
    icon: 'FlaskConical',
  },
  // support_work children
  docs: {
    label: 'Docs',
    bgColor: 'bg-cyan-50 dark:bg-cyan-950/30',
    textColor: 'text-cyan-700 dark:text-cyan-400',
    borderColor: 'border-cyan-200 dark:border-cyan-800',
    icon: 'FileText',
  },
  config: {
    label: 'Config',
    bgColor: 'bg-gray-50 dark:bg-gray-800',
    textColor: 'text-gray-600 dark:text-gray-400',
    borderColor: 'border-gray-200 dark:border-gray-700',
    icon: 'Settings',
  },
  ops: {
    label: 'Ops',
    bgColor: 'bg-indigo-50 dark:bg-indigo-950/30',
    textColor: 'text-indigo-700 dark:text-indigo-400',
    borderColor: 'border-indigo-200 dark:border-indigo-800',
    icon: 'Server',
  },
  // thinking_work children
  planning: {
    label: 'Planning',
    bgColor: 'bg-purple-50 dark:bg-purple-950/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    icon: 'ClipboardList',
  },
  explanation: {
    label: 'Learning',
    bgColor: 'bg-amber-50 dark:bg-amber-950/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    icon: 'Lightbulb',
  },
  architecture: {
    label: 'Architecture',
    bgColor: 'bg-violet-50 dark:bg-violet-950/30',
    textColor: 'text-violet-700 dark:text-violet-400',
    borderColor: 'border-violet-200 dark:border-violet-800',
    icon: 'Blocks',
  },
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
