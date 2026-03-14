/**
 * Content-faithful skeletons for the System Monitor page.
 * Each skeleton matches the exact layout of its real component
 * so the transition from loading → populated is seamless (zero layout shift).
 */

// ── Shared skeleton block ───────────────────────
function Bone({ className }: { className: string }) {
  return <div className={`bg-gray-200 dark:bg-gray-700 rounded ${className}`} />
}

function BoneFaint({ className }: { className: string }) {
  return <div className={`bg-gray-100 dark:bg-gray-800 rounded ${className}`} />
}

// ── Gauge Card Skeleton ─────────────────────────
function GaugeCardSkeleton() {
  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4">
      {/* Label row: "CPU" + "10 cores" */}
      <div className="flex items-baseline justify-between">
        <Bone className="h-3 w-10" />
        <BoneFaint className="h-3 w-16" />
      </div>
      {/* Big value: "60.4%" */}
      <Bone className="h-7 w-20 mt-2" />
      {/* Progress bar */}
      <div className="mt-2 h-1.5 rounded-full bg-gray-100 dark:bg-gray-800 overflow-hidden">
        <BoneFaint className="h-full w-2/5 rounded-full" />
      </div>
    </div>
  )
}

/** Skeleton for the 4-gauge row (CPU, Memory, Disk, Active Sessions). */
export function GaugeRowSkeleton() {
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
      {Array.from({ length: 4 }).map((_, i) => (
        <GaugeCardSkeleton key={`gauge-skel-${i}`} />
      ))}
    </div>
  )
}

// ── Rollup bar skeleton (CPU ████ 2.8%) ─────────
function RollupBarSkeleton({ width = 'w-28' }: { width?: string }) {
  return (
    <div className={`${width} flex items-center gap-2`}>
      <Bone className="h-3 w-8" />
      <div className="flex-1 h-1.5 rounded-full bg-gray-100 dark:bg-gray-800">
        <BoneFaint className="h-full w-1/3 rounded-full" />
      </div>
      <Bone className="h-3 w-10" />
    </div>
  )
}

// ── Session row skeleton (one accordion row) ────
function SessionRowSkeleton() {
  return (
    <div className="border-b border-gray-100 dark:border-gray-800">
      <div className="flex items-center gap-2 px-3 py-2">
        {/* Chevron placeholder */}
        <BoneFaint className="w-5 h-5 rounded" />
        {/* Status dot */}
        <div className="w-2 h-2 rounded-full bg-gray-300 dark:bg-gray-600 shrink-0" />
        {/* Project name + branch + badge */}
        <div className="min-w-0 flex-1 flex items-center gap-2">
          <Bone className="h-4 w-24" />
          <BoneFaint className="h-3.5 w-14" />
          <BoneFaint className="h-4 w-8 rounded" />
        </div>
        {/* Cost */}
        <BoneFaint className="h-3.5 w-10 shrink-0" />
        {/* Proc count */}
        <BoneFaint className="h-4 w-14 rounded-full shrink-0" />
        {/* Turns */}
        <BoneFaint className="h-3.5 w-8 shrink-0" />
        {/* CPU + RAM bars */}
        <div className="flex items-center gap-4 shrink-0 ml-auto">
          <RollupBarSkeleton width="w-56" />
          <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
          <RollupBarSkeleton width="w-56" />
        </div>
      </div>
      {/* "└─ N child procs" hint */}
      <div className="pl-11 pb-1">
        <BoneFaint className="h-3 w-24" />
      </div>
    </div>
  )
}

/** Skeleton for the Claude Sessions panel. */
export function ClaudeSessionsPanelSkeleton({ rows = 3 }: { rows?: number }) {
  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden"
      role="status"
      aria-busy="true"
      aria-label="Loading Claude sessions"
    >
      <span className="sr-only">Loading Claude sessions...</span>
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <Bone className="h-4 w-28" />
        <BoneFaint className="h-5 w-5 rounded-full" />
        <div className="flex items-center gap-4 ml-2">
          <RollupBarSkeleton />
          <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
          <RollupBarSkeleton />
        </div>
        <div className="flex-1" />
        <BoneFaint className="h-3.5 w-16" />
      </div>
      {/* Session rows */}
      <div className="divide-y divide-gray-100 dark:divide-gray-800">
        {Array.from({ length: rows }).map((_, i) => (
          <SessionRowSkeleton key={`session-skel-${i}`} />
        ))}
      </div>
    </div>
  )
}

// ── Process row skeleton ────────────────────────
function ProcessRowSkeleton() {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      {/* Icon */}
      <div className="w-3.5 h-3.5 rounded bg-gray-200 dark:bg-gray-700 shrink-0" />
      {/* Name */}
      <Bone className="h-4 w-28" />
      {/* Process count */}
      <BoneFaint className="h-3.5 w-8" />
      {/* CPU + RAM bars */}
      <div className="flex items-center gap-4 shrink-0 ml-auto">
        <RollupBarSkeleton width="w-56" />
        <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
        <RollupBarSkeleton width="w-56" />
      </div>
    </div>
  )
}

/** Skeleton for the Top Processes panel. */
export function TopProcessesPanelSkeleton({ rows = 5 }: { rows?: number }) {
  return (
    <div
      className="rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 overflow-hidden"
      role="status"
      aria-busy="true"
      aria-label="Loading top processes"
    >
      <span className="sr-only">Loading top processes...</span>
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-gray-100 dark:border-gray-800">
        <Bone className="h-4 w-24" />
        <BoneFaint className="h-5 w-5 rounded-full" />
        <div className="flex items-center gap-4 ml-2">
          <RollupBarSkeleton />
          <div className="w-px h-4 bg-gray-200 dark:bg-gray-700" />
          <RollupBarSkeleton />
        </div>
        <div className="flex-1" />
        <BoneFaint className="h-3.5 w-20" />
      </div>
      {/* Process rows */}
      <div className="flex flex-col">
        {Array.from({ length: rows }).map((_, i) => (
          <ProcessRowSkeleton key={`proc-skel-${i}`} />
        ))}
      </div>
    </div>
  )
}
