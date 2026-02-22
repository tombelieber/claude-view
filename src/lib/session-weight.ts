/**
 * Session weight calculation for visual intensity indicators.
 *
 * Computes a 0-4 tier from session metrics (tokens, turns, files, duration).
 * The composite weight = max across all dimensions, so a session that's
 * extreme in any single dimension gets properly highlighted.
 */

/** Weight tier: 0=tiny, 1=light, 2=medium, 3=heavy, 4=massive */
export type WeightTier = 0 | 1 | 2 | 3 | 4

interface WeightInput {
  totalTokens: number
  userPromptCount: number
  filesEditedCount: number
  durationSeconds: number
  turnCount?: number
  messageCount?: number
}

function tokenTier(tokens: number): WeightTier {
  if (tokens >= 500_000) return 4
  if (tokens >= 200_000) return 3
  if (tokens >= 50_000) return 2
  if (tokens >= 5_000) return 1
  return 0
}

function promptTier(prompts: number): WeightTier {
  if (prompts >= 50) return 4
  if (prompts >= 25) return 3
  if (prompts >= 10) return 2
  if (prompts >= 3) return 1
  return 0
}

function fileTier(files: number): WeightTier {
  if (files >= 16) return 4
  if (files >= 9) return 3
  if (files >= 4) return 2
  if (files >= 1) return 1
  return 0
}

function durationTier(seconds: number): WeightTier {
  if (seconds >= 18000) return 4  // 5+ hours
  if (seconds >= 7200) return 3   // 2+ hours
  if (seconds >= 1800) return 2   // 30+ min
  if (seconds >= 300) return 1    // 5+ min
  return 0
}

export function computeWeight(input: WeightInput): WeightTier {
  return Math.max(
    tokenTier(input.totalTokens),
    promptTier(input.userPromptCount),
    fileTier(input.filesEditedCount),
    durationTier(input.durationSeconds),
  ) as WeightTier
}

/** Tailwind classes for left border accent color per tier */
const BORDER_CLASSES: Record<WeightTier, string> = {
  0: 'border-l-gray-200 dark:border-l-gray-700',
  1: 'border-l-blue-300 dark:border-l-blue-600',
  2: 'border-l-blue-500 dark:border-l-blue-400',
  3: 'border-l-amber-500 dark:border-l-amber-400',
  4: 'border-l-rose-500 dark:border-l-rose-400',
}

export function weightBorderClass(tier: WeightTier): string {
  return BORDER_CLASSES[tier]
}

/** CSS background color for table row dot indicator */
const DOT_CLASSES: Record<WeightTier, string> = {
  0: 'bg-gray-200 dark:bg-gray-700',
  1: 'bg-blue-300 dark:bg-blue-600',
  2: 'bg-blue-500 dark:bg-blue-400',
  3: 'bg-amber-500 dark:bg-amber-400',
  4: 'bg-rose-500 dark:bg-rose-400',
}

export function weightDotClass(tier: WeightTier): string {
  return DOT_CLASSES[tier]
}
