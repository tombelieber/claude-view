/**
 * Message types for conversation rendering.
 *
 * These mirror the Rust-generated types from crates/core but are maintained
 * here so both apps/web and apps/share can import from @claude-view/shared
 * without depending on the web app's generated types.
 */

export type Role =
  | 'user'
  | 'assistant'
  | 'tool_use'
  | 'tool_result'
  | 'system'
  | 'progress'
  | 'summary'

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
