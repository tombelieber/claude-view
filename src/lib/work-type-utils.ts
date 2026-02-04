/**
 * Work type classification utilities.
 *
 * Work types are rule-based classifications (no LLM needed):
 * - Deep Work: duration > 30min, files_edited > 5, LOC > 200
 * - Quick Ask: duration < 5min, turn_count < 3, no edits
 * - Planning: skills contain "brainstorming" or "plan", low edits
 * - Bug Fix: skills contain "debugging", moderate edits
 * - Standard: Everything else
 */

/**
 * Work type classification values from the backend.
 */
export type WorkType = 'deep_work' | 'quick_ask' | 'planning' | 'bug_fix' | 'standard'

/**
 * Configuration for each work type badge style.
 */
export interface WorkTypeConfig {
  label: string
  bgColor: string
  textColor: string
  borderColor: string
  title: string
}

/**
 * Work type configuration map.
 */
export const WORK_TYPE_CONFIG: Record<string, WorkTypeConfig> = {
  deep_work: {
    label: 'Deep Work',
    bgColor: 'bg-blue-50 dark:bg-blue-950/30',
    textColor: 'text-blue-700 dark:text-blue-400',
    borderColor: 'border-blue-200 dark:border-blue-800',
    title: 'Extended coding session (>30 min, many files)',
  },
  quick_ask: {
    label: 'Quick Ask',
    bgColor: 'bg-amber-50 dark:bg-amber-950/30',
    textColor: 'text-amber-700 dark:text-amber-400',
    borderColor: 'border-amber-200 dark:border-amber-800',
    title: 'Brief question (<5 min, no edits)',
  },
  planning: {
    label: 'Planning',
    bgColor: 'bg-purple-50 dark:bg-purple-950/30',
    textColor: 'text-purple-700 dark:text-purple-400',
    borderColor: 'border-purple-200 dark:border-purple-800',
    title: 'Architecture/design discussion',
  },
  bug_fix: {
    label: 'Bug Fix',
    bgColor: 'bg-red-50 dark:bg-red-950/30',
    textColor: 'text-red-700 dark:text-red-400',
    borderColor: 'border-red-200 dark:border-red-800',
    title: 'Debugging session',
  },
  standard: {
    label: 'Standard',
    bgColor: 'bg-gray-50 dark:bg-gray-800',
    textColor: 'text-gray-600 dark:text-gray-400',
    borderColor: 'border-gray-200 dark:border-gray-700',
    title: 'General development session',
  },
}

/**
 * Get work type config for a given work type.
 */
export function getWorkTypeConfig(workType: string): WorkTypeConfig {
  return WORK_TYPE_CONFIG[workType] || WORK_TYPE_CONFIG.standard
}

/**
 * Get all work type options for filter dropdowns.
 */
export function getWorkTypeOptions(): { value: string; label: string }[] {
  return Object.entries(WORK_TYPE_CONFIG).map(([value, config]) => ({
    value,
    label: config.label,
  }))
}
