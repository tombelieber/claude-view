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
/** Neutral style shared by all work types — icon + text label differentiate. */
const NEUTRAL = {
  bgColor: 'bg-gray-50 dark:bg-gray-800',
  textColor: 'text-gray-600 dark:text-gray-400',
  borderColor: 'border-gray-200 dark:border-gray-700',
} as const

export const WORK_TYPE_CONFIG: Record<string, WorkTypeConfig> = {
  deep_work: { label: 'Deep Work',  ...NEUTRAL, title: 'Extended coding session (>30 min, many files)' },
  quick_ask: { label: 'Quick Ask',  ...NEUTRAL, title: 'Brief question (<5 min, no edits)' },
  planning:  { label: 'Planning',   ...NEUTRAL, title: 'Architecture/design discussion' },
  bug_fix:   { label: 'Bug Fix',    ...NEUTRAL, title: 'Debugging session' },
  standard:  { label: 'Standard',   ...NEUTRAL, title: 'General development session' },
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
