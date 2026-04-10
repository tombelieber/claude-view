/**
 * Runtime normalizer for ConversationBlock data from the API.
 *
 * The Rust backend serializes well-typed blocks, but the TS types are
 * hand-written and can drift from reality. Fields that Rust marks as
 * Option<T> may be omitted from JSON; fields from serde_json::Value
 * payloads (system/notice data) have no compile-time guarantees.
 *
 * This normalizer runs ONCE at the fetch boundary — both initial history
 * and paginated older blocks pass through it. Renderers can then trust
 * the TS types without per-field null checks.
 *
 * Policy: truthful defaults only. Arrays default to []. Objects default
 * to {}. Missing IDs are NOT invented — blocks without id are dropped
 * with a console.error so the upstream bug is visible, not hidden.
 */
import type { ConversationBlock } from '../types/blocks'

// ── Helpers ────────────────────────────────────────────────────────

function ensureArray<T>(val: unknown): T[] {
  return Array.isArray(val) ? val : []
}

function ensureRecord(val: unknown): Record<string, unknown> {
  return val != null && typeof val === 'object' && !Array.isArray(val)
    ? (val as Record<string, unknown>)
    : {}
}

function ensureString(val: unknown, fallback = ''): string {
  return typeof val === 'string' ? val : fallback
}

// ── Per-type normalizers ───────────────────────────────────────────

function normalizeAssistant(block: Record<string, unknown>): void {
  block.segments = ensureArray(block.segments)
  block.streaming = block.streaming ?? false
}

function normalizeInteraction(block: Record<string, unknown>): void {
  // Rust synthesizer sets request_id: None for historical blocks (70 across
  // 5,970 real sessions). These are always resolved: true, so requestId is
  // display-only — '' is truthful (no active request exists).
  if (block.requestId == null) block.requestId = ''
  block.resolved = block.resolved ?? false
  block.data = block.data ?? {}
}

function normalizeTurnBoundary(block: Record<string, unknown>): void {
  block.usage = ensureRecord(block.usage)
  block.modelUsage = ensureRecord(block.modelUsage)
  block.permissionDenials = ensureArray(block.permissionDenials)
  // Nested: error.messages must be an array if error exists
  if (block.error != null && typeof block.error === 'object') {
    const err = block.error as Record<string, unknown>
    err.messages = ensureArray(err.messages)
    err.subtype = ensureString(err.subtype, 'error_during_execution')
  }
}

function normalizeNotice(block: Record<string, unknown>): void {
  block.data = block.data ?? {}
  const data = block.data as Record<string, unknown>
  switch (block.variant) {
    case 'auth_status':
      data.output = ensureArray(data.output)
      break
    case 'prompt_suggestion':
      data.suggestion = ensureString(data.suggestion)
      break
  }
}

function normalizeSystem(block: Record<string, unknown>): void {
  block.data = block.data ?? {}
  const data = block.data as Record<string, unknown>
  switch (block.variant) {
    case 'session_init':
      data.tools = ensureArray(data.tools)
      data.agents = ensureArray(data.agents)
      data.model = ensureString(data.model, '?')
      data.permissionMode = ensureString(data.permissionMode)
      data.cwd = ensureString(data.cwd)
      break
    case 'task_started':
      data.taskId = ensureString(data.taskId)
      data.description = ensureString(data.description)
      break
    case 'stream_delta':
      data.messageId = ensureString(data.messageId)
      break
    case 'files_saved':
      data.files = ensureArray(data.files)
      data.failed = ensureArray(data.failed)
      break
    case 'worktree_state':
      data.worktreeSession = ensureRecord(data.worktreeSession)
      break
  }
}

function normalizeProgress(block: Record<string, unknown>): void {
  block.data = block.data ?? {}
}

function normalizeTeamTranscript(block: Record<string, unknown>): void {
  block.speakers = ensureArray(block.speakers)
  block.entries = ensureArray(block.entries)
}

// ── Main entry point ──────────────────────────────────────────────

/**
 * Normalize a single block in place. Returns the same reference.
 * Call at the fetch boundary before blocks enter the store.
 */
export function normalizeBlock(block: ConversationBlock): ConversationBlock {
  const raw = block as Record<string, unknown>

  switch (block.type) {
    case 'assistant':
      normalizeAssistant(raw)
      break
    case 'interaction':
      normalizeInteraction(raw)
      break
    case 'turn_boundary':
      normalizeTurnBoundary(raw)
      break
    case 'notice':
      normalizeNotice(raw)
      break
    case 'system':
      normalizeSystem(raw)
      break
    case 'progress':
      normalizeProgress(raw)
      break
    case 'team_transcript':
      normalizeTeamTranscript(raw)
      break
  }

  return block
}

/**
 * Normalize an array of blocks. Filters out malformed entries:
 * - Non-objects, missing `type` field
 * - Missing `id` field (logs error — don't hide upstream bugs)
 */
export function normalizeBlocks(blocks: unknown[]): ConversationBlock[] {
  return (blocks ?? [])
    .filter((b): b is ConversationBlock => {
      if (b == null || typeof b !== 'object') return false
      const obj = b as Record<string, unknown>
      if (typeof obj.type !== 'string') return false
      // Missing id = upstream bug. Drop and surface the error.
      if (!obj.id || typeof obj.id !== 'string') {
        console.error('[normalizeBlocks] dropped block with missing id — upstream bug:', {
          type: obj.type,
          variant: obj.variant,
          keys: Object.keys(obj),
        })
        return false
      }
      return true
    })
    .map(normalizeBlock)
}
