/**
 * Unified cost/token category color scheme
 *
 * Single source of truth for all cost and token breakdown components.
 * Ensures consistency across:
 * - CostBreakdownCard
 * - TokenBreakdown
 * - CostBreakdown (live sessions)
 * - CostTooltip
 * - CostTokenPopover
 */

export const COST_CATEGORY_COLORS = {
  /** Fresh input tokens — baseline cost */
  input: {
    light: 'bg-gray-400 text-gray-900',
    dark: 'dark:bg-gray-500 dark:text-gray-100',
    text: 'text-gray-600 dark:text-gray-400',
    dot: 'bg-gray-400 dark:bg-gray-500',
  },

  /** Model output tokens — primary cost driver */
  output: {
    light: 'bg-blue-600 text-white',
    dark: 'dark:bg-blue-400 dark:text-blue-950',
    text: 'text-blue-600 dark:text-blue-400',
    dot: 'bg-blue-600 dark:bg-blue-400',
  },

  /** Prompt cache hits — savings indicator (green) */
  cacheRead: {
    light: 'bg-emerald-500 text-white',
    dark: 'dark:bg-emerald-400 dark:text-emerald-950',
    text: 'text-emerald-600 dark:text-emerald-400',
    dot: 'bg-emerald-500 dark:bg-emerald-400',
  },

  /** Prompt cache writes — deferred cost */
  cacheWrite: {
    light: 'bg-amber-500 text-white',
    dark: 'dark:bg-amber-400 dark:text-amber-950',
    text: 'text-amber-600 dark:text-amber-400',
    dot: 'bg-amber-500 dark:bg-amber-400',
  },

  /** Positive indicator — cache savings amount */
  savings: {
    light: 'bg-emerald-500 text-white',
    dark: 'dark:bg-emerald-400 dark:text-emerald-950',
    text: 'text-emerald-600 dark:text-emerald-400',
    dot: 'bg-emerald-500 dark:bg-emerald-400',
  },

  /** Warning state — unavailable pricing or unpriced usage */
  warning: {
    light: 'bg-amber-50 border-amber-200 text-amber-700',
    dark: 'dark:bg-amber-950/20 dark:border-amber-900/60 dark:text-amber-300',
    text: 'text-amber-600 dark:text-amber-400',
    dot: 'bg-amber-500 dark:bg-amber-400',
  },

  /** Error state — cache expired, failed requests */
  error: {
    light: 'text-red-600',
    dark: 'dark:text-red-400',
    text: 'text-red-600 dark:text-red-400',
    dot: 'bg-red-500 dark:bg-red-400',
  },

  /** Neutral text — labels, secondary info */
  neutral: {
    light: 'text-gray-500',
    dark: 'dark:text-gray-400',
    text: 'text-gray-500 dark:text-gray-400',
    dot: 'bg-gray-400 dark:bg-gray-500',
  },
} as const

/** Category to color mapping for segment displays */
export const COST_SEGMENT_CONFIG = [
  {
    key: 'inputCostUsd' as const,
    label: 'Fresh Input',
    color: COST_CATEGORY_COLORS.input,
  },
  {
    key: 'outputCostUsd' as const,
    label: 'Output',
    color: COST_CATEGORY_COLORS.output,
  },
  {
    key: 'cacheReadCostUsd' as const,
    label: 'Cache Read',
    color: COST_CATEGORY_COLORS.cacheRead,
  },
  {
    key: 'cacheCreationCostUsd' as const,
    label: 'Cache Write',
    color: COST_CATEGORY_COLORS.cacheWrite,
  },
] as const

export const TOKEN_SEGMENT_CONFIG = [
  {
    key: 'totalInputTokens' as const,
    label: 'Fresh Input',
    color: COST_CATEGORY_COLORS.input,
  },
  {
    key: 'totalOutputTokens' as const,
    label: 'Output',
    color: COST_CATEGORY_COLORS.output,
  },
  {
    key: 'cacheReadTokens' as const,
    label: 'Cache Read',
    color: COST_CATEGORY_COLORS.cacheRead,
  },
  {
    key: 'cacheCreationTokens' as const,
    label: 'Cache Write',
    color: COST_CATEGORY_COLORS.cacheWrite,
  },
] as const
