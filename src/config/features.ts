/**
 * Feature Flags for Safe Rollback
 *
 * All features are enabled by default and can be disabled via environment variables.
 * This enables safe rollback if a feature causes issues post-release.
 *
 * Environment Variables (set to 'false' to disable):
 * - VITE_FEATURE_TIME_RANGE: Dashboard time range selector (7d/30d/90d/all-time)
 * - VITE_FEATURE_AI_GENERATION: AI generation stats display
 * - VITE_FEATURE_STORAGE_OVERVIEW: Storage overview display
 *
 * Usage:
 *   import { FEATURES } from '@/config/features'
 *
 *   // Conditional rendering
 *   {FEATURES.timeRange && <TimeRangeSelector />}
 *
 *   // Conditional logic
 *   if (FEATURES.aiGeneration) {
 *     fetchAiStats()
 *   }
 *
 * To disable a feature in development:
 *   VITE_FEATURE_TIME_RANGE=false bun run dev
 *
 * To disable a feature in production build:
 *   VITE_FEATURE_TIME_RANGE=false bun run build
 */

/**
 * Feature flags object - all features enabled by default.
 * Set corresponding VITE_FEATURE_* env var to 'false' to disable.
 */
export const FEATURES = {
  /**
   * Dashboard time range selector (7d/30d/90d/all-time)
   * Allows users to filter dashboard metrics by time period.
   */
  timeRange: import.meta.env.VITE_FEATURE_TIME_RANGE !== 'false',

  /**
   * AI generation stats display
   * Shows AI generation metrics in the dashboard.
   */
  aiGeneration: import.meta.env.VITE_FEATURE_AI_GENERATION !== 'false',

  /**
   * Storage overview display
   * Shows storage usage metrics in the dashboard.
   */
  storageOverview: import.meta.env.VITE_FEATURE_STORAGE_OVERVIEW !== 'false',
} as const

/**
 * Type for feature flag names
 */
export type FeatureName = keyof typeof FEATURES

/**
 * Check if a feature is enabled
 * @param feature - The feature name to check
 * @returns true if the feature is enabled
 */
export function isFeatureEnabled(feature: FeatureName): boolean {
  return FEATURES[feature]
}

/**
 * Get all enabled features
 * @returns Array of enabled feature names
 */
export function getEnabledFeatures(): FeatureName[] {
  return (Object.keys(FEATURES) as FeatureName[]).filter(
    (key) => FEATURES[key]
  )
}

/**
 * Get all disabled features
 * @returns Array of disabled feature names
 */
export function getDisabledFeatures(): FeatureName[] {
  return (Object.keys(FEATURES) as FeatureName[]).filter(
    (key) => !FEATURES[key]
  )
}
