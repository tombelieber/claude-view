// apps/web/src/lib/notify.ts

/**
 * NOTIFICATION DECISION TREE
 * ══════════════════════════
 * Is it a confirmation of something the user just did?
 *   -> toast.success() / toast()          [MICRO or STANDARD]
 *
 * Is it an error from something the user just tried?
 *   -> toast.error()                      [EXTENDED, add description]
 *   Can the user retry?
 *     -> add action: { label: 'Retry', onClick }
 *
 * Is it a degraded system state (auth, data quality, stale index)?
 *   -> <Banner variant="..." layout="bar|inline">
 *
 * Is it a destructive or irreversible action?
 *   -> Radix AlertDialog
 *
 * Does the user need to provide input (sign in, grant permission)?
 *   -> Radix Dialog
 */
export const TOAST_DURATION = {
  /** Zero-stakes confirmations: "copied!", "saved!", "removed" */
  micro: 2000,
  /** Sonner default — actionable success with context: "archived" + Undo, "sync complete" + stats */
  standard: 4000,
  /** Errors requiring comprehension + optional recovery action */
  extended: 6000,
  /** Long-lived informational toasts (pattern alerts, onboarding hints) — auto-dismiss but generous */
  persistent: 12000,
} as const
