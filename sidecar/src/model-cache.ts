// sidecar/src/model-cache.ts
// In-memory cache for SDK-provided supported models.
// Populated on sidecar startup via V1 query() API, refreshed hourly.
// Serves GET /supported-models.
//
// Why V1 query()? V2 SDKSession doesn't expose supportedModels() — only the
// V1 Query interface has initializationResult() → { models: ModelInfo[] }.
// The query spawns a CLI subprocess, reads init data, then interrupts.

import { type ModelInfo, query } from '@anthropic-ai/claude-agent-sdk'
import { findClaudeExecutable } from './cli-path.js'

interface ModelCacheState {
  models: ModelInfo[]
  updatedAt: number // epoch ms
}

let cache: ModelCacheState | null = null
let refreshInFlight = false

/** Get full cache state including updatedAt timestamp. */
export function getCacheState(): { models: ModelInfo[]; updatedAt: number | null } {
  return {
    models: cache?.models ?? [],
    updatedAt: cache?.updatedAt ?? null,
  }
}

/**
 * Update model cache directly from a live session's supportedModels() result.
 * Called on every session create/resume — the SDK is the source of truth.
 * No-op if the model list hasn't changed.
 */
export function updateModelCacheFromSession(models: ModelInfo[]): void {
  if (!models || models.length === 0) return

  const newIds = models.map((m) => m.value).sort()
  const oldIds = cache ? cache.models.map((m) => m.value).sort() : []

  if (JSON.stringify(newIds) !== JSON.stringify(oldIds)) {
    // biome-ignore lint/suspicious/noConsoleLog: sidecar server logging convention
    console.log(
      `[model-cache] Updated from session: ${models.length} models (was ${cache?.models.length ?? 0})`,
    )
    cache = { models, updatedAt: Date.now() }
  }
}

/** Promise.race with a timeout — prevents initializationResult() from hanging forever. */
function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms),
    ),
  ])
}

/**
 * Refresh the model cache using the V1 query() API.
 *
 * Creates a V1 query to read initialization data (which includes the model list),
 * then immediately interrupts. This spawns a CLI subprocess, so it's called
 * sparingly: once on startup, then hourly.
 *
 * Fire-and-forget: never throws, logs errors.
 */
async function refreshModelCache(): Promise<void> {
  if (refreshInFlight) {
    console.log('[model-cache] Skipped: refresh already in flight')
    return
  }
  refreshInFlight = true

  let q: ReturnType<typeof query> | null = null

  try {
    q = query({ prompt: '', options: { pathToClaudeCodeExecutable: findClaudeExecutable() } })

    // Timeout guards against initializationResult() hanging (e.g. CLI not found,
    // auth issues, network problems). Without this, refreshInFlight stays true
    // permanently, blocking all future refreshes.
    const initResult = await withTimeout(q.initializationResult(), 30_000, 'initializationResult')
    const models = initResult.models

    // Interrupt immediately — we only needed the init data, not a conversation turn.
    await q.interrupt()
    q = null

    if (!models || models.length === 0) {
      console.log('[model-cache] SDK returned empty model list — keeping existing cache')
      return
    }

    // Compare by model IDs — only update cache if the list changed
    const newIds = models.map((m) => m.value).sort()
    const oldIds = cache ? cache.models.map((m) => m.value).sort() : []

    if (JSON.stringify(newIds) !== JSON.stringify(oldIds)) {
      console.log(
        `[model-cache] Updated: ${models.length} models (was ${cache?.models.length ?? 0})`,
      )
      cache = { models, updatedAt: Date.now() }
    }
  } catch (err) {
    console.error('[model-cache] Failed to refresh:', err instanceof Error ? err.message : err)
    // Best-effort cleanup if query is still alive after timeout/error
    if (q) {
      try {
        await q.interrupt()
      } catch {
        /* already dead */
      }
    }
  } finally {
    refreshInFlight = false
  }
}

/**
 * Start the model cache refresh lifecycle.
 * Runs once immediately on startup, then every hour.
 */
export function startModelCacheRefresh(): void {
  refreshModelCache()
  setInterval(refreshModelCache, 60 * 60 * 1000)
}
