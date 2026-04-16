// sidecar/src/memory-monitor.ts
// Periodic heap/RSS telemetry + threshold warnings.
//
// Motivation: without this, sidecar OOM crashes were diagnosed only by the
// Rust reconciler noticing the child had died (#54). By then the crash
// context is gone — we can't distinguish a slow leak from a sudden spike.
// Logging every 60s to stderr gives us a timeline alongside existing
// server logs in ~/.claude-view/logs, so the next OOM report includes
// heap growth history.

const SAMPLE_INTERVAL_MS = 60_000
/** RSS threshold above which we start warning on every sample. 1 GiB default
 *  is generous — typical sidecar under 10-20 active sessions sits at ~200 MB. */
const RSS_WARN_BYTES = 1024 * 1024 * 1024
/** Growth (RSS delta since last warn) that triggers a warn regardless of
 *  absolute RSS. Catches slow leaks that cross the warn threshold between
 *  samples. 100 MiB per minute is unusual even under heavy load. */
const RSS_GROWTH_WARN_BYTES = 100 * 1024 * 1024

function formatMiB(bytes: number): string {
  return `${(bytes / 1024 / 1024).toFixed(1)} MiB`
}

export interface MemoryMonitorHandle {
  stop(): void
}

/**
 * Start periodic heap telemetry. Logs a single summary line per sample and
 * escalates to a warn when RSS crosses thresholds.
 *
 * `activeCount` should return the current number of live sessions so we can
 * correlate memory growth with session load.
 */
export function startMemoryMonitor(activeCount: () => number): MemoryMonitorHandle {
  let lastWarnRss = 0
  const timer = setInterval(() => {
    const mem = process.memoryUsage()
    const active = activeCount()
    const line = `[memory] rss=${formatMiB(mem.rss)} heap_used=${formatMiB(mem.heapUsed)} heap_total=${formatMiB(mem.heapTotal)} external=${formatMiB(mem.external)} sessions=${active}`

    const crossedAbsolute = mem.rss > RSS_WARN_BYTES
    const crossedGrowth = lastWarnRss > 0 && mem.rss - lastWarnRss > RSS_GROWTH_WARN_BYTES
    if (crossedAbsolute || crossedGrowth) {
      console.warn(
        `${line} -- WARN ${crossedAbsolute ? 'rss > threshold' : `rss grew ${formatMiB(mem.rss - lastWarnRss)} since last warn`}`,
      )
      lastWarnRss = mem.rss
    } else {
      console.log(line)
    }
  }, SAMPLE_INTERVAL_MS)
  // Don't keep the event loop alive for a background timer.
  timer.unref?.()
  return {
    stop() {
      clearInterval(timer)
    },
  }
}
