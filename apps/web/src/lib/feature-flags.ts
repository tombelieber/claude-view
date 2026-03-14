/**
 * Feature flags for gating incomplete or broken features.
 * Flip to `true` to re-enable.
 */
export const FEATURES = {
  /** AI-powered session classification (classify buttons, banner, settings section) */
  classify: false,
  /** Insights tab inside Analytics */
  insights: false,
  /** Chat input UI (resume/send messages in browser). */
  chat: true,
} as const
