/// <reference types="vite/client" />

/**
 * Type declarations for Vite environment variables.
 *
 * Feature flags can be disabled by setting the corresponding env var to 'false'.
 * All features are enabled by default when the env var is undefined.
 */
interface ImportMetaEnv {
  /**
   * Dashboard time range selector (7d/30d/90d/all-time)
   * Set to 'false' to disable this feature.
   */
  readonly VITE_FEATURE_TIME_RANGE?: string

  /**
   * Enhanced heatmap with detailed tooltips
   * Set to 'false' to disable this feature.
   */
  readonly VITE_FEATURE_HEATMAP_TOOLTIP?: string

  /**
   * Redesigned sync UI with progress indicators
   * Set to 'false' to disable this feature.
   */
  readonly VITE_FEATURE_SYNC_REDESIGN?: string

  /**
   * AI generation stats display
   * Set to 'false' to disable this feature.
   */
  readonly VITE_FEATURE_AI_GENERATION?: string

  /**
   * Storage overview display
   * Set to 'false' to disable this feature.
   */
  readonly VITE_FEATURE_STORAGE_OVERVIEW?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
