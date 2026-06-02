interface WatchingBannerProps {
  /** Open the take-over flow (fork this CLI session into an SDK-driven branch). */
  onTakeover: () => void
}

/**
 * Shown above the input when the panel is OBSERVING a session running in the
 * Claude Code CLI (read-only mirror — the CLI is the source of truth). The
 * "Take over" button is the deliberate, single-action entry into CONTROL mode:
 * it forks the conversation into a new SDK-owned branch while the original CLI
 * session keeps running. One writer per lineage — we never write into the CLI's.
 */
export function WatchingBanner({ onTakeover }: WatchingBannerProps) {
  return (
    <div className="mb-2 flex items-center justify-between gap-3 rounded-lg border border-blue-200 dark:border-blue-800/50 bg-blue-50 dark:bg-blue-950/30 px-3 py-2">
      <p className="text-xs text-blue-600/80 dark:text-blue-400/70">
        Running in Claude Code CLI — you&apos;re watching live.
      </p>
      <button
        type="button"
        onClick={onTakeover}
        className="flex-shrink-0 rounded-md bg-blue-600 px-3 py-1 text-xs font-medium text-white hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-1 dark:focus:ring-offset-gray-900"
      >
        Take over
      </button>
    </div>
  )
}
