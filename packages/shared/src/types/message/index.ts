/**
 * Message types for conversation rendering.
 *
 * These mirror the Rust-generated types from crates/core but are maintained
 * here so both apps/web and apps/share can import from @claude-view/shared
 * without depending on the web app's generated types.
 */

export type Role = 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress'

export type ToolCall = {
  name: string
  count: number
  input?: unknown
  category?: string | null
}

export type Message = {
  role: Role
  content: string
  timestamp?: string | null
  tool_calls?: Array<ToolCall> | null
  thinking?: string | null
  uuid?: string | null
  parent_uuid?: string | null
  metadata?: unknown
  category?: string | null
}

export type SessionMetadata = {
  totalMessages: number
  toolCallCount: number
}

export type ParsedSession = {
  messages: Array<Message>
  metadata: SessionMetadata
}

export type ToolUsageSummary = {
  name: string
  count: number
}

export type ShareCommit = {
  hash: string
  message: string
}

/** Extended metadata included in share blobs (optional for backward compat) */
export type ShareSessionMetadata = {
  sessionId: string
  projectName?: string
  primaryModel?: string
  durationSeconds?: number
  totalInputTokens?: number
  totalOutputTokens?: number
  userPromptCount?: number
  toolCallCount?: number
  filesReadCount?: number
  filesEditedCount?: number
  commitCount?: number
  gitBranch?: string
  sessionTitle?: string
  /** Optional because Rust uses skip_serializing_if = "Vec::is_empty" — absent when empty */
  toolsUsed?: ToolUsageSummary[]
  filesRead?: string[]
  filesEdited?: string[]
  commits?: ShareCommit[]
}

/** The full payload in a share blob (superset of ParsedSession) */
export type SharePayload = {
  messages: Array<Message>
  metadata: SessionMetadata
  shareMetadata?: ShareSessionMetadata
}
