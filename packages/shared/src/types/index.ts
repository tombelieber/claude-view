export * from './relay'
export * from './sidecar-protocol'
export * from './blocks'
export type {
  AgentState,
  AgentStateGroup,
  CacheStatus,
  ControlBinding,
  CostBreakdown,
  HookEvent,
  JsonValue,
  LiveSession,
  ProgressItem,
  ProgressSource,
  ProgressStatus,
  SessionStatus,
  SubAgentInfo,
  SubAgentStatus,
  TokenUsage,
  ToolUsed,
} from './generated'

// -- Supabase domain re-exports --
// NEVER edit supabase.generated.ts by hand — re-run
// `supabase gen types typescript --project-id $SUPABASE_DEV_PROJECT_REF > supabase.generated.ts`
// after any schema change.

import type { Database } from './supabase.generated'

export type Tables<T extends keyof Database['public']['Tables']> =
  Database['public']['Tables'][T]['Row']
export type InsertTables<T extends keyof Database['public']['Tables']> =
  Database['public']['Tables'][T]['Insert']
export type UpdateTables<T extends keyof Database['public']['Tables']> =
  Database['public']['Tables'][T]['Update']

/** A paired device row from public.devices. */
export type Device = Tables<'devices'>
/** Fields required to insert a new device. */
export type DeviceInsert = InsertTables<'devices'>
/** Fields updateable on an existing device (only display_name for clients). */
export type DeviceUpdate = UpdateTables<'devices'>

/** A single entry in the audit log. */
export type DeviceEvent = Tables<'device_events'>

/** A pending pairing offer (service-role access only). */
export type PairingOffer = Tables<'pairing_offers'>

// -- Stable platform enum (mirrors CHECK constraint in schema) --

export type DevicePlatform = 'mac' | 'ios' | 'android' | 'web'

// -- Stable revoke reason enum (mirrors CHECK constraint in schema) --

export type RevokeReason = 'user_action' | 'bulk_terminate' | 'inactivity_gc' | 'admin'

// -- Stable audit event enum (mirrors CHECK constraint in schema) --

export type DeviceEventType =
  | 'paired'
  | 'unpaired'
  | 'revoked'
  | 'expired'
  | 'connected'
  | 'disconnected'
  | 'renamed'

export type { Database }
