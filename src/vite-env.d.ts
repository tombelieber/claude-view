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
