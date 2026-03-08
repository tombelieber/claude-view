/**
 * Relay protocol types — event envelopes and command types for the
 * Mac ↔ Phone relay WebSocket protocol.
 *
 * Session data uses the generated `LiveSession` type (from Rust's
 * `#[derive(TS)]`). Only relay-specific protocol types are defined here.
 */

import type { LiveSession } from './generated'

// -- Session events (matches state.rs SessionEvent, tagged enum) --
// The Mac sends these as encrypted payloads. After decryption, each
// message is one of these event types.

export interface SessionDiscoveredEvent {
  type: 'session_discovered'
  session: LiveSession
}

export interface SessionUpdatedEvent {
  type: 'session_updated'
  session: LiveSession
}

export interface SessionCompletedEvent {
  type: 'session_completed'
  sessionId: string
}

/** Mac → Phone: live output stream */
export interface RelayOutputStream {
  type: 'output'
  sessionId: string
  chunks: RelayOutputChunk[]
}

export interface RelayOutputChunk {
  role: 'assistant' | 'tool' | 'user'
  text?: string
  name?: string
  path?: string
}

/** Phone → Mac: command */
export interface RelayCommand {
  type: 'command'
  action: string
  sessionId?: string
  [key: string]: unknown
}

/**
 * Union of all relay message types.
 *
 * NOTE: The Mac's relay_client.rs sends individual LiveSession objects
 * (not wrapped in an event envelope) for the initial snapshot. For
 * broadcast events, it sends the tagged SessionEvent format. The phone's
 * relay hook must handle both:
 *   1. Raw LiveSession (no `type` field) — initial snapshot item
 *   2. Tagged events with `type` field — ongoing updates
 */
export type RelayMessage =
  | SessionDiscoveredEvent
  | SessionUpdatedEvent
  | SessionCompletedEvent
  | RelayOutputStream
  | RelayCommand
