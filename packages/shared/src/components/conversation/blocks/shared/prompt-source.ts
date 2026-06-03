import type { BadgeColor } from './StatusBadge'

// Display metadata for a Claude Code `promptSource` tag — the verbatim origin
// of a user prompt: "typed" (human), "sdk" (Agent SDK injected), or "system"
// (system-generated). This is raw CLI data, not inferred, so surfacing it is
// trust-safe (no confidence gate needed). Absent on older sessions.

export interface PromptSourceDisplay {
  /** Human-facing short label. */
  label: string
  /**
   * Whether to surface in the polished chat view. The default human-authored
   * case ("typed") is hidden as noise — a pill that says "you typed this" on
   * every message carries no signal. Non-default origins (sdk/system) are the
   * only ones worth flagging to a reader.
   */
  chatVisible: boolean
  /** Badge tint for the developer (debug) view. */
  color: BadgeColor
}

const KNOWN: Record<string, PromptSourceDisplay> = {
  typed: { label: 'typed', chatVisible: false, color: 'gray' },
  sdk: { label: 'SDK', chatVisible: true, color: 'cyan' },
  system: { label: 'system', chatVisible: true, color: 'amber' },
}

/**
 * Map a raw `promptSource` value to its display metadata, or `null` when the
 * field is absent. Unknown future values are surfaced verbatim (Zero Data
 * Loss) rather than silently dropped — forward-compatible with new CLI prompt
 * origins a future Claude Code release might introduce.
 */
export function promptSourceDisplay(source?: string | null): PromptSourceDisplay | null {
  if (!source) return null
  return KNOWN[source] ?? { label: source, chatVisible: true, color: 'gray' }
}
