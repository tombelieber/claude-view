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
 * Rule: fix the TYPE GAP at the boundary, not in the consumer.
 */
import type { ConversationBlock } from '../types/blocks'

// ── Helpers ────────────────────────────────────────────────────────

let _idCounter = 0

function uniqueFallbackId(prefix: string): string {
  return `${prefix}-${Date.now()}-${++_idCounter}`
}

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
  // Rust: request_id is Option<String> — can be None/omitted
  // Each interaction needs a unique requestId because useInteractionHandlers()
  // keys response state by requestId — shared '' would cross-contaminate cards
  if (block.requestId == null) block.requestId = uniqueFallbackId('req')
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
  // Variant-specific data normalization
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
  // All blocks must have an id
  const raw = block as Record<string, unknown>
  if (!raw.id || typeof raw.id !== 'string') raw.id = uniqueFallbackId('block')

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
 * Normalize an array of blocks. Filters out any entries that aren't
 * objects with a `type` field (malformed data from API).
 */
export function normalizeBlocks(blocks: unknown[]): ConversationBlock[] {
  return (blocks ?? [])
    .filter(
      (b): b is ConversationBlock =>
        b != null &&
        typeof b === 'object' &&
        'type' in b &&
        typeof (b as Record<string, unknown>).type === 'string',
    )
    .map(normalizeBlock)
}
