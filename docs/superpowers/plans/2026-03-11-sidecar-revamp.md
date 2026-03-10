# Sidecar Revamp Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the sidecar to handle ALL 22 Agent SDK v0.2.72 message types, producing 27 protocol events with zero silent drops.

**Architecture:** One long-lived `stream()` loop per session (not per-turn). Pure event-mapper translates SDK messages to typed protocol events. Permission handler forwards full context (toolUseID, suggestions, decisionReason, blockedPath, agentID). Rust proxy unchanged.

**Tech Stack:** TypeScript (Node.js 20+), Agent SDK `@anthropic-ai/claude-agent-sdk` v0.2.72, Hono HTTP framework, `ws` WebSocket library, Vitest for tests.

**Spec:** `docs/superpowers/specs/2026-03-11-sidecar-revamp-design.md`

---

## File Map

### Sidecar (`sidecar/src/`)

| File | Action | Responsibility |
|---|---|---|
| `protocol.ts` | **Create** (replaces `types.ts`) | All 27 server events + 7 client messages, fully typed discriminated unions |
| `event-mapper.ts` | **Create** | Pure function: `SDKMessage → ProtocolEvent[]`. Exhaustive switch, no side effects. |
| `event-mapper.test.ts` | **Create** | Tests for every SDK message type mapping |
| `permission-handler.ts` | **Create** | `canUseTool` callback factory, 4 pending maps, resolve/drain methods |
| `permission-handler.test.ts` | **Create** | Tests for routing, timeout, abort, drain |
| `session-registry.ts` | **Create** | `Map<controlId, ControlSession>`, lookup by controlId/sessionId, list, cleanup |
| `sdk-session.ts` | **Create** | `createControlSession()`, `resumeControlSession()`, long-lived stream loop, `sendMessage()` |
| `routes.ts` | **Create** (replaces `control.ts`) | HTTP endpoints: create, resume, list, list-available, prompt, terminate |
| `ws-handler.ts` | **Modify** | Handle all 7 client message types, ring buffer replay |
| `index.ts` | **Modify** | Wire new modules, keep shutdown/socket logic |
| `health.ts` | Keep | No changes |
| `ring-buffer.ts` | Keep | No changes |
| `ring-buffer.test.ts` | Keep | No changes |
| `types.ts` | **Delete** | Replaced by `protocol.ts` |
| `session-manager.ts` | **Delete** | Split into session-registry, sdk-session, event-mapper, permission-handler |
| `control.ts` | **Delete** | Replaced by `routes.ts` |
| `control.test.ts` | **Delete** | Replaced by new tests |

### Frontend (`apps/web/src/`)

| File | Action | Responsibility |
|---|---|---|
| `types/control.ts` | **Modify** | Expand from 11 to 27 server event types, add new UI state types |
| `hooks/use-control-session.ts` | **Modify** | Handle 27 event types in switch, add new state fields |
| `hooks/use-available-sessions.ts` | **Create** | Hook for session picker (`GET /control/available-sessions`) |

---

## Chunk 1: Protocol Types & Event Mapper

### Task 1: Create `sidecar/src/protocol.ts`

The foundation — every other file imports from here.

**Files:**
- Create: `sidecar/src/protocol.ts`

- [ ] **Step 1: Write server event types (sidecar → frontend)**

```typescript
// sidecar/src/protocol.ts
// Complete protocol: sidecar ↔ frontend over WebSocket
// Every SDK message type maps to one or more of these events.

// ─── Server → Client Events ───────────────────────────────────────

export interface AssistantText {
  type: 'assistant_text'
  text: string
  messageId: string // SDK msg.uuid
  parentToolUseId: string | null
}

export interface AssistantThinking {
  type: 'assistant_thinking'
  thinking: string
  messageId: string
  parentToolUseId: string | null
}

export interface AssistantError {
  type: 'assistant_error'
  error: 'authentication_failed' | 'billing_error' | 'rate_limit' | 'invalid_request' | 'server_error' | 'unknown' | 'max_output_tokens'
  messageId: string
}

export interface StreamDelta {
  type: 'stream_delta'
  event: unknown // BetaRawMessageStreamEvent — forward compat
  messageId: string
}

export interface ToolUseStart {
  type: 'tool_use_start'
  toolName: string
  toolInput: Record<string, unknown>
  toolUseId: string
  messageId: string
  parentToolUseId: string | null
}

export interface ToolUseResult {
  type: 'tool_use_result'
  toolUseId: string
  output: string
  isError: boolean
  isReplay: boolean
}

export interface ToolProgress {
  type: 'tool_progress'
  toolUseId: string
  toolName: string
  elapsedSeconds: number
  parentToolUseId: string | null
  taskId?: string
}

export interface ToolSummary {
  type: 'tool_summary'
  summary: string
  precedingToolUseIds: string[]
}

export interface ModelUsageInfo {
  inputTokens: number
  outputTokens: number
  cacheReadInputTokens: number
  cacheCreationInputTokens: number
  webSearchRequests: number
  costUSD: number
  contextWindow: number
  maxOutputTokens: number
}

export interface TurnComplete {
  type: 'turn_complete'
  totalCostUsd: number
  numTurns: number
  durationMs: number
  durationApiMs: number
  usage: Record<string, number> // NonNullableUsage flattened
  modelUsage: Record<string, ModelUsageInfo>
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  result: string
  structuredOutput?: unknown
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
}

export interface TurnError {
  type: 'turn_error'
  subtype: 'error_during_execution' | 'error_max_turns' | 'error_max_budget_usd' | 'error_max_structured_output_retries'
  errors: string[]
  permissionDenials: { toolName: string; toolUseId: string; toolInput: Record<string, unknown> }[]
  totalCostUsd: number
  numTurns: number
  durationMs: number
  usage: Record<string, number>
  modelUsage: Record<string, ModelUsageInfo>
  stopReason: string | null
  fastModeState?: 'off' | 'cooldown' | 'on'
}

export interface SessionInit {
  type: 'session_init'
  tools: string[]
  model: string
  mcpServers: { name: string; status: string }[]
  permissionMode: string
  slashCommands: string[]
  claudeCodeVersion: string
  cwd: string
  agents: string[]
  skills: string[]
  outputStyle: string
}

export interface SessionStatus {
  type: 'session_status'
  status: 'compacting' | null
  permissionMode?: string
}

export interface ContextCompacted {
  type: 'context_compacted'
  trigger: 'manual' | 'auto'
  preTokens: number
}

export interface ElicitationComplete {
  type: 'elicitation_complete'
  mcpServerName: string
  elicitationId: string
}

export interface RateLimit {
  type: 'rate_limit'
  status: 'allowed' | 'allowed_warning' | 'rejected'
  resetsAt?: number
  utilization?: number
  rateLimitType?: string
  overageStatus?: string
  isUsingOverage?: boolean
}

export interface TaskStarted {
  type: 'task_started'
  taskId: string
  toolUseId?: string
  description: string
  taskType?: string
  prompt?: string
}

export interface TaskProgressEvent {
  type: 'task_progress'
  taskId: string
  toolUseId?: string
  description: string
  lastToolName?: string
  summary?: string
  usage: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface TaskNotification {
  type: 'task_notification'
  taskId: string
  toolUseId?: string
  status: 'completed' | 'failed' | 'stopped'
  outputFile: string
  summary: string
  usage?: { totalTokens: number; toolUses: number; durationMs: number }
}

export interface HookEvent {
  type: 'hook_event'
  phase: 'started' | 'progress' | 'response'
  hookId: string
  hookName: string
  hookEventName: string
  stdout?: string
  stderr?: string
  output?: string
  exitCode?: number
  outcome?: 'success' | 'error' | 'cancelled'
}

export interface AuthStatus {
  type: 'auth_status'
  isAuthenticating: boolean
  output: string[]
  error?: string
}

export interface FilesSaved {
  type: 'files_saved'
  files: { filename: string; fileId: string }[]
  failed: { filename: string; error: string }[]
  processedAt: string
}

export interface PromptSuggestion {
  type: 'prompt_suggestion'
  suggestion: string
}

export interface CommandOutput {
  type: 'command_output'
  content: string
}

export interface UnknownSdkEvent {
  type: 'unknown_sdk_event'
  sdkType: string
  raw: unknown
}

export interface SessionClosed {
  type: 'session_closed'
  reason: string
}

// ─── Interactive Cards (from canUseTool) ──────────────────────────

export interface PermissionRequest {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  toolUseID: string
  suggestions?: unknown[] // PermissionUpdate[] from SDK
  decisionReason?: string
  blockedPath?: string
  agentID?: string
  timeoutMs: number
}

export interface AskQuestion {
  type: 'ask_question'
  requestId: string
  questions: {
    question: string
    header: string
    options: { label: string; description: string; markdown?: string }[]
    multiSelect: boolean
  }[]
}

export interface PlanApproval {
  type: 'plan_approval'
  requestId: string
  planData: Record<string, unknown>
}

export interface Elicitation {
  type: 'elicitation'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
}

// ─── Infrastructure ───────────────────────────────────────────────

export interface ErrorEvent {
  type: 'error'
  message: string
  fatal: boolean
}

export interface PongEvent {
  type: 'pong'
}

export interface HeartbeatConfig {
  type: 'heartbeat_config'
  intervalMs: number
}

// ─── Union Types ──────────────────────────────────────────────────

export type ServerEvent =
  // Assistant output
  | AssistantText
  | AssistantThinking
  | AssistantError
  | StreamDelta
  // Tool execution
  | ToolUseStart
  | ToolUseResult
  | ToolProgress
  | ToolSummary
  // Turn lifecycle
  | TurnComplete
  | TurnError
  // Session
  | SessionInit
  | SessionStatus
  | SessionClosed
  // Context
  | ContextCompacted
  | RateLimit
  // Tasks (subagents)
  | TaskStarted
  | TaskProgressEvent
  | TaskNotification
  // System
  | HookEvent
  | AuthStatus
  | FilesSaved
  | CommandOutput
  | PromptSuggestion
  // Interactive cards
  | PermissionRequest
  | AskQuestion
  | PlanApproval
  | Elicitation
  // Infrastructure
  | ErrorEvent
  | PongEvent
  | UnknownSdkEvent

export type SequencedEvent = ServerEvent & { seq: number }

// ─── Client → Server Messages ─────────────────────────────────────

export interface UserMessage {
  type: 'user_message'
  content: string
}

export interface PermissionResponse {
  type: 'permission_response'
  requestId: string
  allowed: boolean
  updatedPermissions?: unknown[] // echo back suggestions for "always allow"
}

export interface QuestionResponse {
  type: 'question_response'
  requestId: string
  answers: Record<string, string>
}

export interface PlanResponseMsg {
  type: 'plan_response'
  requestId: string
  approved: boolean
  feedback?: string
}

export interface ElicitationResponse {
  type: 'elicitation_response'
  requestId: string
  response: string
}

export interface ResumeMsg {
  type: 'resume'
  lastSeq: number
}

export interface PingMsg {
  type: 'ping'
}

export type ClientMessage =
  | UserMessage
  | PermissionResponse
  | QuestionResponse
  | PlanResponseMsg
  | ElicitationResponse
  | ResumeMsg
  | PingMsg

// ─── HTTP Request/Response Types ──────────────────────────────────

export interface CreateSessionRequest {
  model: string
  permissionMode?: string
  allowedTools?: string[]
  disallowedTools?: string[]
  projectPath?: string
  initialMessage?: string
}

export interface ResumeSessionRequest {
  sessionId: string
  model?: string
  permissionMode?: string
  projectPath?: string
}

export interface PromptRequest {
  message: string
  model: string
  permissionMode?: string
}

export interface SessionResponse {
  controlId: string
  sessionId: string
  status: 'created' | 'resumed' | 'already_active'
}

export interface AvailableSession {
  sessionId: string
  summary: string
  lastModified: number
  fileSize: number
  customTitle?: string
  firstPrompt?: string
  gitBranch?: string
  cwd?: string
}

export interface ActiveSession {
  controlId: string
  sessionId: string
  state: string
  turnCount: number
  totalCostUsd: number | null
  startedAt: number
}

export interface HealthResponse {
  status: 'ok'
  activeSessions: number
  uptime: number
}
```

- [ ] **Step 2: Verify types compile**

Run: `cd sidecar && npx tsc --noEmit src/protocol.ts`
Expected: Clean (0 errors)

- [ ] **Step 3: Commit**

```bash
git add sidecar/src/protocol.ts
git commit -m "feat(sidecar): add complete protocol types — 27 server events + 7 client messages"
```

---

### Task 2: Create `sidecar/src/event-mapper.ts` with tests

Pure translation function. No side effects, no state — easy to test.

**Files:**
- Create: `sidecar/src/event-mapper.ts`
- Create: `sidecar/src/event-mapper.test.ts`

- [ ] **Step 1: Write failing tests for assistant message mapping**

```typescript
// sidecar/src/event-mapper.test.ts
import { describe, expect, it } from 'vitest'
import { mapSdkMessage } from './event-mapper.js'
import type { SDKMessage } from '@anthropic-ai/claude-agent-sdk'

// Helper to create a minimal SDKAssistantMessage
function assistantMsg(content: { type: string; [k: string]: unknown }[], opts?: { error?: string }): SDKMessage {
  return {
    type: 'assistant',
    message: { content, role: 'assistant', id: 'msg_1', type: 'message', model: 'claude-sonnet-4-20250514', stop_reason: null, stop_sequence: null, usage: { input_tokens: 0, output_tokens: 0 } },
    parent_tool_use_id: null,
    error: opts?.error,
    uuid: '00000000-0000-0000-0000-000000000001' as `${string}-${string}-${string}-${string}-${string}`,
    session_id: 'sess-1',
  } as unknown as SDKMessage
}

describe('mapSdkMessage', () => {
  describe('assistant messages', () => {
    it('maps text blocks to assistant_text', () => {
      const events = mapSdkMessage(assistantMsg([{ type: 'text', text: 'Hello world' }]))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_text',
        text: 'Hello world',
      })
    })

    it('maps tool_use blocks to tool_use_start', () => {
      const events = mapSdkMessage(assistantMsg([
        { type: 'tool_use', id: 'tu_1', name: 'Read', input: { file_path: '/foo' } },
      ]))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'tool_use_start',
        toolName: 'Read',
        toolUseId: 'tu_1',
      })
    })

    it('maps thinking blocks to assistant_thinking', () => {
      const events = mapSdkMessage(assistantMsg([{ type: 'thinking', thinking: 'Let me think...' }]))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_thinking',
        thinking: 'Let me think...',
      })
    })

    it('maps multiple content blocks to multiple events', () => {
      const events = mapSdkMessage(assistantMsg([
        { type: 'text', text: 'First' },
        { type: 'tool_use', id: 'tu_1', name: 'Bash', input: { command: 'ls' } },
        { type: 'text', text: 'Second' },
      ]))
      expect(events).toHaveLength(3)
      expect(events[0].type).toBe('assistant_text')
      expect(events[1].type).toBe('tool_use_start')
      expect(events[2].type).toBe('assistant_text')
    })

    it('emits assistant_error when error field is set', () => {
      const events = mapSdkMessage(assistantMsg([], { error: 'rate_limit' }))
      expect(events).toHaveLength(1)
      expect(events[0]).toMatchObject({
        type: 'assistant_error',
        error: 'rate_limit',
      })
    })
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd sidecar && npx vitest run src/event-mapper.test.ts`
Expected: FAIL — `event-mapper.ts` does not exist

- [ ] **Step 3: Write event-mapper implementation**

```typescript
// sidecar/src/event-mapper.ts
// Pure translation: SDKMessage → ServerEvent[]
// Exhaustive switch, zero silent drops, no side effects.

import type { SDKMessage } from '@anthropic-ai/claude-agent-sdk'
import type {
  AssistantError,
  AssistantText,
  AssistantThinking,
  AuthStatus,
  CommandOutput,
  ContextCompacted,
  ElicitationComplete,
  FilesSaved,
  HookEvent,
  ModelUsageInfo,
  PromptSuggestion,
  RateLimit,
  ServerEvent,
  SessionClosed,
  SessionInit,
  SessionStatus,
  StreamDelta,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
  ToolProgress,
  ToolSummary,
  ToolUseResult,
  ToolUseStart,
  TurnComplete,
  TurnError,
  UnknownSdkEvent,
} from './protocol.js'

type AnyMsg = SDKMessage & Record<string, unknown>

export function mapSdkMessage(msg: SDKMessage): ServerEvent[] {
  const m = msg as AnyMsg

  switch (m.type) {
    case 'assistant':
      return mapAssistant(m)
    case 'user':
      return mapUser(m)
    case 'result':
      return mapResult(m)
    case 'system':
      return mapSystem(m)
    case 'stream_event':
      return mapStreamEvent(m)
    case 'tool_progress':
      return mapToolProgress(m)
    case 'rate_limit_event':
      return mapRateLimit(m)
    case 'auth_status':
      return mapAuthStatus(m)
    case 'tool_use_summary':
      return mapToolSummary(m)
    case 'prompt_suggestion':
      return mapPromptSuggestion(m)
    default:
      return [{ type: 'unknown_sdk_event', sdkType: m.type ?? 'undefined', raw: m } satisfies UnknownSdkEvent]
  }
}

// ─── Assistant ────────────────────────────────────────────────────

function mapAssistant(m: AnyMsg): ServerEvent[] {
  const msgId = String(m.uuid ?? '')
  const parentToolUseId = (m.parent_tool_use_id as string | null) ?? null

  // Check error field first — may have empty content on error
  if (m.error) {
    return [{ type: 'assistant_error', error: m.error, messageId: msgId } satisfies AssistantError]
  }

  const message = m.message as { content: { type: string; [k: string]: unknown }[] }
  if (!message?.content) return []

  const events: ServerEvent[] = []
  for (const block of message.content) {
    switch (block.type) {
      case 'text':
        events.push({
          type: 'assistant_text',
          text: (block.text as string) ?? '',
          messageId: msgId,
          parentToolUseId,
        } satisfies AssistantText)
        break
      case 'tool_use':
        events.push({
          type: 'tool_use_start',
          toolName: (block.name as string) ?? '',
          toolInput: (block.input as Record<string, unknown>) ?? {},
          toolUseId: (block.id as string) ?? '',
          messageId: msgId,
          parentToolUseId,
        } satisfies ToolUseStart)
        break
      case 'thinking':
        events.push({
          type: 'assistant_thinking',
          thinking: (block.thinking as string) ?? '',
          messageId: msgId,
          parentToolUseId,
        } satisfies AssistantThinking)
        break
      // Ignore other block types (e.g. images) — log if needed
    }
  }
  return events
}

// ─── User (tool results) ─────────────────────────────────────────

function mapUser(m: AnyMsg): ServerEvent[] {
  const isReplay = 'replay' in m && Boolean(m.replay)
  const message = m.message as { content: unknown }
  if (!message?.content) return []

  const content = message.content
  // content can be string or array of content blocks
  if (typeof content === 'string') return [] // plain text user message, no protocol event needed

  if (!Array.isArray(content)) return []

  const events: ServerEvent[] = []
  for (const block of content) {
    const b = block as Record<string, unknown>
    if (b.type === 'tool_result') {
      // Extract output text from content (can be string or nested blocks)
      let output = ''
      if (typeof b.content === 'string') {
        output = b.content
      } else if (Array.isArray(b.content)) {
        output = (b.content as { type: string; text?: string }[])
          .filter((c) => c.type === 'text')
          .map((c) => c.text ?? '')
          .join('\n')
      }

      events.push({
        type: 'tool_use_result',
        toolUseId: (b.tool_use_id as string) ?? '',
        output,
        isError: Boolean(b.is_error),
        isReplay,
      } satisfies ToolUseResult)
    }
  }
  return events
}

// ─── Result ──────────────────────────────────────────────────────

function mapResult(m: AnyMsg): ServerEvent[] {
  const usage = (m.usage ?? {}) as Record<string, number>
  const modelUsage = mapModelUsage((m.modelUsage ?? {}) as Record<string, Record<string, unknown>>)
  const denials = ((m.permission_denials ?? []) as { tool_name: string; tool_use_id: string; tool_input: Record<string, unknown> }[])
    .map((d) => ({ toolName: d.tool_name, toolUseId: d.tool_use_id, toolInput: d.tool_input }))

  if (m.subtype === 'success') {
    return [{
      type: 'turn_complete',
      totalCostUsd: (m.total_cost_usd as number) ?? 0,
      numTurns: (m.num_turns as number) ?? 0,
      durationMs: (m.duration_ms as number) ?? 0,
      durationApiMs: (m.duration_api_ms as number) ?? 0,
      usage,
      modelUsage,
      permissionDenials: denials,
      result: (m.result as string) ?? '',
      structuredOutput: m.structured_output,
      stopReason: (m.stop_reason as string | null) ?? null,
      fastModeState: m.fast_mode_state as 'off' | 'cooldown' | 'on' | undefined,
    } satisfies TurnComplete]
  }

  // Error result
  return [{
    type: 'turn_error',
    subtype: (m.subtype as TurnError['subtype']) ?? 'error_during_execution',
    errors: (m.errors as string[]) ?? [],
    permissionDenials: denials,
    totalCostUsd: (m.total_cost_usd as number) ?? 0,
    numTurns: (m.num_turns as number) ?? 0,
    durationMs: (m.duration_ms as number) ?? 0,
    usage,
    modelUsage,
    stopReason: (m.stop_reason as string | null) ?? null,
    fastModeState: m.fast_mode_state as 'off' | 'cooldown' | 'on' | undefined,
  } satisfies TurnError]
}

function mapModelUsage(raw: Record<string, Record<string, unknown>>): Record<string, ModelUsageInfo> {
  const result: Record<string, ModelUsageInfo> = {}
  for (const [model, usage] of Object.entries(raw)) {
    result[model] = {
      inputTokens: (usage.inputTokens as number) ?? 0,
      outputTokens: (usage.outputTokens as number) ?? 0,
      cacheReadInputTokens: (usage.cacheReadInputTokens as number) ?? 0,
      cacheCreationInputTokens: (usage.cacheCreationInputTokens as number) ?? 0,
      webSearchRequests: (usage.webSearchRequests as number) ?? 0,
      costUSD: (usage.costUSD as number) ?? 0,
      contextWindow: (usage.contextWindow as number) ?? 0,
      maxOutputTokens: (usage.maxOutputTokens as number) ?? 0,
    }
  }
  return result
}

// ─── System (route by subtype) ───────────────────────────────────

function mapSystem(m: AnyMsg): ServerEvent[] {
  switch (m.subtype) {
    case 'init':
      return [{
        type: 'session_init',
        tools: (m.tools as string[]) ?? [],
        model: (m.model as string) ?? '',
        mcpServers: (m.mcp_servers as { name: string; status: string }[]) ?? [],
        permissionMode: (m.permissionMode as string) ?? 'default',
        slashCommands: (m.slash_commands as string[]) ?? [],
        claudeCodeVersion: (m.claude_code_version as string) ?? '',
        cwd: (m.cwd as string) ?? '',
        agents: (m.agents as string[]) ?? [],
        skills: (m.skills as string[]) ?? [],
        outputStyle: (m.output_style as string) ?? '',
      } satisfies SessionInit]

    case 'status':
      return [{
        type: 'session_status',
        status: (m.status as 'compacting' | null) ?? null,
        permissionMode: m.permissionMode as string | undefined,
      } satisfies SessionStatus]

    case 'compact_boundary':
      return [{
        type: 'context_compacted',
        trigger: ((m.compact_metadata as Record<string, unknown>)?.trigger as 'manual' | 'auto') ?? 'auto',
        preTokens: ((m.compact_metadata as Record<string, unknown>)?.pre_tokens as number) ?? 0,
      } satisfies ContextCompacted]

    case 'elicitation_complete':
      return [{
        type: 'elicitation_complete',
        mcpServerName: (m.mcp_server_name as string) ?? '',
        elicitationId: (m.elicitation_id as string) ?? '',
      } satisfies ElicitationComplete]

    case 'task_started':
      return [{
        type: 'task_started',
        taskId: (m.task_id as string) ?? '',
        toolUseId: m.tool_use_id as string | undefined,
        description: (m.description as string) ?? '',
        taskType: m.task_type as string | undefined,
        prompt: m.prompt as string | undefined,
      } satisfies TaskStarted]

    case 'task_progress':
      return [{
        type: 'task_progress',
        taskId: (m.task_id as string) ?? '',
        toolUseId: m.tool_use_id as string | undefined,
        description: (m.description as string) ?? '',
        lastToolName: m.last_tool_name as string | undefined,
        summary: m.summary as string | undefined,
        usage: {
          totalTokens: ((m.usage as Record<string, number>)?.total_tokens) ?? 0,
          toolUses: ((m.usage as Record<string, number>)?.tool_uses) ?? 0,
          durationMs: ((m.usage as Record<string, number>)?.duration_ms) ?? 0,
        },
      } satisfies TaskProgressEvent]

    case 'task_notification':
      return [{
        type: 'task_notification',
        taskId: (m.task_id as string) ?? '',
        toolUseId: m.tool_use_id as string | undefined,
        status: (m.status as 'completed' | 'failed' | 'stopped') ?? 'failed',
        outputFile: (m.output_file as string) ?? '',
        summary: (m.summary as string) ?? '',
        usage: m.usage ? {
          totalTokens: ((m.usage as Record<string, number>).total_tokens) ?? 0,
          toolUses: ((m.usage as Record<string, number>).tool_uses) ?? 0,
          durationMs: ((m.usage as Record<string, number>).duration_ms) ?? 0,
        } : undefined,
      } satisfies TaskNotification]

    case 'hook_started':
      return [{
        type: 'hook_event',
        phase: 'started',
        hookId: (m.hook_id as string) ?? '',
        hookName: (m.hook_name as string) ?? '',
        hookEventName: (m.hook_event as string) ?? '',
      } satisfies HookEvent]

    case 'hook_progress':
      return [{
        type: 'hook_event',
        phase: 'progress',
        hookId: (m.hook_id as string) ?? '',
        hookName: (m.hook_name as string) ?? '',
        hookEventName: (m.hook_event as string) ?? '',
        stdout: m.stdout as string | undefined,
        stderr: m.stderr as string | undefined,
        output: m.output as string | undefined,
      } satisfies HookEvent]

    case 'hook_response':
      return [{
        type: 'hook_event',
        phase: 'response',
        hookId: (m.hook_id as string) ?? '',
        hookName: (m.hook_name as string) ?? '',
        hookEventName: (m.hook_event as string) ?? '',
        stdout: m.stdout as string | undefined,
        stderr: m.stderr as string | undefined,
        output: m.output as string | undefined,
        exitCode: m.exit_code as number | undefined,
        outcome: m.outcome as 'success' | 'error' | 'cancelled' | undefined,
      } satisfies HookEvent]

    case 'files_persisted':
      return [{
        type: 'files_saved',
        files: ((m.files ?? []) as { filename: string; file_id: string }[]).map((f) => ({
          filename: f.filename,
          fileId: f.file_id,
        })),
        failed: ((m.failed ?? []) as { filename: string; error: string }[]),
        processedAt: (m.processed_at as string) ?? '',
      } satisfies FilesSaved]

    case 'local_command_output':
      return [{
        type: 'command_output',
        content: (m.content as string) ?? '',
      } satisfies CommandOutput]

    default:
      return [{ type: 'unknown_sdk_event', sdkType: `system:${m.subtype}`, raw: m } satisfies UnknownSdkEvent]
  }
}

// ─── Other top-level types ───────────────────────────────────────

function mapStreamEvent(m: AnyMsg): ServerEvent[] {
  return [{
    type: 'stream_delta',
    event: m.event,
    messageId: String(m.uuid ?? ''),
  } satisfies StreamDelta]
}

function mapToolProgress(m: AnyMsg): ServerEvent[] {
  return [{
    type: 'tool_progress',
    toolUseId: (m.tool_use_id as string) ?? '',
    toolName: (m.tool_name as string) ?? '',
    elapsedSeconds: (m.elapsed_time_seconds as number) ?? 0,
    parentToolUseId: (m.parent_tool_use_id as string | null) ?? null,
    taskId: m.task_id as string | undefined,
  } satisfies ToolProgress]
}

function mapRateLimit(m: AnyMsg): ServerEvent[] {
  const info = (m.rate_limit_info ?? {}) as Record<string, unknown>
  return [{
    type: 'rate_limit',
    status: (info.status as RateLimit['status']) ?? 'allowed',
    resetsAt: info.resetsAt as number | undefined,
    utilization: info.utilization as number | undefined,
    rateLimitType: info.rateLimitType as string | undefined,
    overageStatus: info.overageStatus as string | undefined,
    isUsingOverage: info.isUsingOverage as boolean | undefined,
  } satisfies RateLimit]
}

function mapAuthStatus(m: AnyMsg): ServerEvent[] {
  return [{
    type: 'auth_status',
    isAuthenticating: Boolean(m.isAuthenticating),
    output: (m.output as string[]) ?? [],
    error: m.error as string | undefined,
  } satisfies AuthStatus]
}

function mapToolSummary(m: AnyMsg): ServerEvent[] {
  return [{
    type: 'tool_summary',
    summary: (m.summary as string) ?? '',
    precedingToolUseIds: (m.preceding_tool_use_ids as string[]) ?? [],
  } satisfies ToolSummary]
}

function mapPromptSuggestion(m: AnyMsg): ServerEvent[] {
  return [{
    type: 'prompt_suggestion',
    suggestion: (m.suggestion as string) ?? '',
  } satisfies PromptSuggestion]
}
```

- [ ] **Step 4: Run tests**

Run: `cd sidecar && npx vitest run src/event-mapper.test.ts`
Expected: All 5 tests PASS

- [ ] **Step 5: Add tests for result, user (tool results), and system messages**

Add to `event-mapper.test.ts`:

```typescript
describe('result messages', () => {
  it('maps success result to turn_complete with real data', () => {
    const msg = {
      type: 'result',
      subtype: 'success',
      total_cost_usd: 0.0042,
      num_turns: 3,
      duration_ms: 12000,
      duration_api_ms: 8000,
      usage: { input_tokens: 1000, output_tokens: 500 },
      modelUsage: {
        'claude-sonnet-4-20250514': {
          inputTokens: 1000, outputTokens: 500,
          cacheReadInputTokens: 200, cacheCreationInputTokens: 0,
          webSearchRequests: 0, costUSD: 0.0042,
          contextWindow: 200000, maxOutputTokens: 16384,
        },
      },
      permission_denials: [],
      result: 'Done!',
      stop_reason: 'end_turn',
      fast_mode_state: 'off',
      uuid: '00000000-0000-0000-0000-000000000002',
      session_id: 'sess-1',
    } as unknown as SDKMessage

    const events = mapSdkMessage(msg)
    expect(events).toHaveLength(1)
    const e = events[0] as TurnComplete
    expect(e.type).toBe('turn_complete')
    expect(e.totalCostUsd).toBe(0.0042)
    expect(e.numTurns).toBe(3)
    expect(e.modelUsage['claude-sonnet-4-20250514'].contextWindow).toBe(200000)
    expect(e.result).toBe('Done!')
    expect(e.fastModeState).toBe('off')
  })

  it('maps error result to turn_error', () => {
    const msg = {
      type: 'result',
      subtype: 'error_max_turns',
      total_cost_usd: 0.05,
      num_turns: 10,
      duration_ms: 60000,
      errors: ['Max turns reached'],
      permission_denials: [{ tool_name: 'Bash', tool_use_id: 'tu_1', tool_input: {} }],
      usage: {},
      modelUsage: {},
      uuid: 'u2',
      session_id: 's1',
    } as unknown as SDKMessage

    const events = mapSdkMessage(msg)
    expect(events).toHaveLength(1)
    expect(events[0].type).toBe('turn_error')
    expect((events[0] as TurnError).subtype).toBe('error_max_turns')
    expect((events[0] as TurnError).errors).toEqual(['Max turns reached'])
    expect((events[0] as TurnError).permissionDenials).toHaveLength(1)
  })
})

describe('user messages (tool results)', () => {
  it('maps tool_result blocks to tool_use_result', () => {
    const msg = {
      type: 'user',
      message: {
        role: 'user',
        content: [
          { type: 'tool_result', tool_use_id: 'tu_1', content: 'file contents here', is_error: false },
        ],
      },
      parent_tool_use_id: null,
      uuid: 'u3',
      session_id: 's1',
    } as unknown as SDKMessage

    const events = mapSdkMessage(msg)
    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({
      type: 'tool_use_result',
      toolUseId: 'tu_1',
      output: 'file contents here',
      isError: false,
      isReplay: false,
    })
  })
})

describe('system messages', () => {
  it('maps init to session_init', () => {
    const msg = {
      type: 'system',
      subtype: 'init',
      tools: ['Read', 'Edit', 'Bash'],
      model: 'claude-sonnet-4-20250514',
      mcp_servers: [{ name: 'context7', status: 'connected' }],
      permissionMode: 'default',
      slash_commands: ['/help', '/clear'],
      claude_code_version: '1.2.3',
      cwd: '/home/user/project',
      agents: ['code-reviewer'],
      skills: ['commit'],
      output_style: 'normal',
      uuid: 'u4',
      session_id: 's1',
    } as unknown as SDKMessage

    const events = mapSdkMessage(msg)
    expect(events).toHaveLength(1)
    expect(events[0].type).toBe('session_init')
    const e = events[0] as SessionInit
    expect(e.tools).toEqual(['Read', 'Edit', 'Bash'])
    expect(e.model).toBe('claude-sonnet-4-20250514')
    expect(e.cwd).toBe('/home/user/project')
  })

  it('maps unknown system subtype to unknown_sdk_event', () => {
    const msg = { type: 'system', subtype: 'future_thing', uuid: 'u5', session_id: 's1' } as unknown as SDKMessage
    const events = mapSdkMessage(msg)
    expect(events[0].type).toBe('unknown_sdk_event')
  })
})

describe('unknown message types', () => {
  it('maps completely unknown type to unknown_sdk_event', () => {
    const msg = { type: 'brand_new_thing', data: 42 } as unknown as SDKMessage
    const events = mapSdkMessage(msg)
    expect(events[0]).toMatchObject({
      type: 'unknown_sdk_event',
      sdkType: 'brand_new_thing',
    })
  })
})
```

- [ ] **Step 6: Run all event-mapper tests**

Run: `cd sidecar && npx vitest run src/event-mapper.test.ts`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add sidecar/src/event-mapper.ts sidecar/src/event-mapper.test.ts
git commit -m "feat(sidecar): add event-mapper — pure SDK→protocol translation for all 22 message types"
```

---

## Chunk 2: Permission Handler, Session Registry & SDK Session

### Task 3: Create `sidecar/src/permission-handler.ts` with tests

**Files:**
- Create: `sidecar/src/permission-handler.ts`
- Create: `sidecar/src/permission-handler.test.ts`

- [ ] **Step 1: Write failing tests**

```typescript
// sidecar/src/permission-handler.test.ts
import { describe, expect, it, vi } from 'vitest'
import { PermissionHandler } from './permission-handler.js'

describe('PermissionHandler', () => {
  it('routes AskUserQuestion to ask_question event', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => { events.push(e) }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'AskUserQuestion',
      { questions: [{ question: 'Pick one', header: 'H', options: [], multiSelect: false }] },
      { signal, toolUseID: 'tu_1' },
      emit,
    )

    // Should have emitted ask_question event
    expect(events).toHaveLength(1)
    expect((events[0] as Record<string, unknown>).type).toBe('ask_question')

    // Resolve the question
    const requestId = (events[0] as Record<string, string>).requestId
    handler.resolveQuestion(requestId, { 'Pick one': 'Option A' })

    const result = await promise
    expect(result.behavior).toBe('allow')
  })

  it('routes generic tools to permission_request with full context', async () => {
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => { events.push(e) }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'rm -rf /' },
      { signal, toolUseID: 'tu_2', decisionReason: 'dangerous command', blockedPath: '/' },
      emit,
    )

    expect(events).toHaveLength(1)
    const req = events[0] as Record<string, unknown>
    expect(req.type).toBe('permission_request')
    expect(req.toolUseID).toBe('tu_2')
    expect(req.decisionReason).toBe('dangerous command')
    expect(req.blockedPath).toBe('/')

    // Deny
    const requestId = req.requestId as string
    handler.resolvePermission(requestId, false)

    const result = await promise
    expect(result.behavior).toBe('deny')
  })

  it('auto-denies after timeout', async () => {
    vi.useFakeTimers()
    const handler = new PermissionHandler()
    const events: unknown[] = []
    const emit = (e: unknown) => { events.push(e) }

    const signal = new AbortController().signal
    const promise = handler.handleCanUseTool(
      'Bash',
      { command: 'ls' },
      { signal, toolUseID: 'tu_3' },
      emit,
      { timeoutMs: 1000 },
    )

    vi.advanceTimersByTime(1001)
    const result = await promise
    expect(result.behavior).toBe('deny')
    expect(result.message).toContain('timed out')

    vi.useRealTimers()
  })

  it('drainAll denies all pending', () => {
    const handler = new PermissionHandler()
    const promises: Promise<unknown>[] = []
    const emit = () => {}
    const signal = new AbortController().signal

    promises.push(handler.handleCanUseTool('Bash', {}, { signal, toolUseID: '1' }, emit))
    promises.push(handler.handleCanUseTool('Edit', {}, { signal, toolUseID: '2' }, emit))

    handler.drainAll()
    // All should resolve
    return Promise.all(promises)
  })
})
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd sidecar && npx vitest run src/permission-handler.test.ts`
Expected: FAIL

- [ ] **Step 3: Write implementation**

```typescript
// sidecar/src/permission-handler.ts
import type { PermissionResult } from '@anthropic-ai/claude-agent-sdk'
import type {
  AskQuestion,
  Elicitation,
  PermissionRequest,
  PlanApproval,
  ServerEvent,
} from './protocol.js'

interface CanUseToolOptions {
  signal: AbortSignal
  suggestions?: unknown[]
  blockedPath?: string
  decisionReason?: string
  toolUseID: string
  agentID?: string
}

interface PendingPermission {
  resolve: (result: PermissionResult) => void
  timer: ReturnType<typeof setTimeout> | null
}

interface PendingQuestion {
  resolve: (result: PermissionResult) => void
}

interface PendingPlan {
  resolve: (result: PermissionResult) => void
}

interface PendingElicitation {
  resolve: (result: PermissionResult) => void
}

export class PermissionHandler {
  private permissions = new Map<string, PendingPermission>()
  private questions = new Map<string, PendingQuestion>()
  private plans = new Map<string, PendingPlan>()
  private elicitations = new Map<string, PendingElicitation>()

  async handleCanUseTool(
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
    config?: { timeoutMs?: number },
  ): Promise<PermissionResult> {
    const requestId = crypto.randomUUID()
    const timeoutMs = config?.timeoutMs ?? 60_000

    if (toolName === 'AskUserQuestion') {
      return this.handleQuestion(requestId, input, options, emit)
    }
    if (toolName === 'ExitPlanMode') {
      return this.handlePlan(requestId, input, options, emit)
    }
    // MCP elicitation — detect by shape (has prompt field, not a standard tool)
    if (input.prompt && typeof input.prompt === 'string' && !['Read', 'Edit', 'Write', 'Bash', 'Grep', 'Glob'].includes(toolName)) {
      return this.handleElicitation(requestId, toolName, input, options, emit)
    }

    return this.handlePermission(requestId, toolName, input, options, emit, timeoutMs)
  }

  private handleQuestion(
    requestId: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.questions.set(requestId, {
        resolve: (result) => resolve(result),
      })

      options.signal.addEventListener('abort', () => {
        if (this.questions.has(requestId)) {
          this.questions.delete(requestId)
          resolve({ behavior: 'deny', message: 'Question aborted' })
        }
      }, { once: true })

      emit({
        type: 'ask_question',
        requestId,
        questions: input.questions as AskQuestion['questions'],
      } satisfies AskQuestion)
    })
  }

  private handlePlan(
    requestId: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.plans.set(requestId, {
        resolve: (result) => resolve(result),
      })

      options.signal.addEventListener('abort', () => {
        if (this.plans.has(requestId)) {
          this.plans.delete(requestId)
          resolve({ behavior: 'deny', message: 'Plan approval aborted' })
        }
      }, { once: true })

      emit({
        type: 'plan_approval',
        requestId,
        planData: input,
      } satisfies PlanApproval)
    })
  }

  private handleElicitation(
    requestId: string,
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      this.elicitations.set(requestId, {
        resolve: (result) => resolve(result),
      })

      options.signal.addEventListener('abort', () => {
        if (this.elicitations.has(requestId)) {
          this.elicitations.delete(requestId)
          resolve({ behavior: 'deny', message: 'Elicitation aborted' })
        }
      }, { once: true })

      emit({
        type: 'elicitation',
        requestId,
        toolName,
        toolInput: input,
      } satisfies Elicitation)
    })
  }

  private handlePermission(
    requestId: string,
    toolName: string,
    input: Record<string, unknown>,
    options: CanUseToolOptions,
    emit: (event: ServerEvent) => void,
    timeoutMs: number,
  ): Promise<PermissionResult> {
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        if (this.permissions.has(requestId)) {
          this.permissions.delete(requestId)
          resolve({ behavior: 'deny', message: `Permission for ${toolName} timed out` })
        }
      }, timeoutMs)

      this.permissions.set(requestId, {
        resolve: (result) => resolve(result),
        timer,
      })

      options.signal.addEventListener('abort', () => {
        const pending = this.permissions.get(requestId)
        if (pending) {
          if (pending.timer) clearTimeout(pending.timer)
          this.permissions.delete(requestId)
          resolve({ behavior: 'deny', message: 'Request aborted' })
        }
      }, { once: true })

      emit({
        type: 'permission_request',
        requestId,
        toolName,
        toolInput: input,
        toolUseID: options.toolUseID,
        suggestions: options.suggestions,
        decisionReason: options.decisionReason,
        blockedPath: options.blockedPath,
        agentID: options.agentID,
        timeoutMs,
      } satisfies PermissionRequest)
    })
  }

  // ─── Resolve methods (called from WS handler) ──────────────────

  resolvePermission(requestId: string, allowed: boolean, updatedPermissions?: unknown[]): boolean {
    const pending = this.permissions.get(requestId)
    if (!pending) return false
    if (pending.timer) clearTimeout(pending.timer)
    this.permissions.delete(requestId)
    pending.resolve(
      allowed
        ? { behavior: 'allow', updatedInput: undefined, updatedPermissions: updatedPermissions as any }
        : { behavior: 'deny', message: 'User denied' },
    )
    return true
  }

  resolveQuestion(requestId: string, answers: Record<string, string>): boolean {
    const pending = this.questions.get(requestId)
    if (!pending) return false
    this.questions.delete(requestId)
    pending.resolve({ behavior: 'allow', updatedInput: { answers } })
    return true
  }

  resolvePlan(requestId: string, approved: boolean, feedback?: string): boolean {
    const pending = this.plans.get(requestId)
    if (!pending) return false
    this.plans.delete(requestId)
    pending.resolve(
      approved
        ? { behavior: 'allow' }
        : { behavior: 'deny', message: feedback ?? 'Plan rejected' },
    )
    return true
  }

  resolveElicitation(requestId: string, response: string): boolean {
    const pending = this.elicitations.get(requestId)
    if (!pending) return false
    this.elicitations.delete(requestId)
    pending.resolve({ behavior: 'allow', updatedInput: { response } })
    return true
  }

  /** Drain all pending — deny permissions, reject plans, empty answers/responses */
  drainAll(): void {
    for (const [, p] of this.permissions) {
      if (p.timer) clearTimeout(p.timer)
      p.resolve({ behavior: 'deny', message: 'Session closing' })
    }
    this.permissions.clear()
    for (const [, p] of this.questions) p.resolve({ behavior: 'allow', updatedInput: { answers: {} } })
    this.questions.clear()
    for (const [, p] of this.plans) p.resolve({ behavior: 'deny', message: 'Session closing' })
    this.plans.clear()
    for (const [, p] of this.elicitations) p.resolve({ behavior: 'allow', updatedInput: { response: '' } })
    this.elicitations.clear()
  }

  /** Drain only the maps that have no auto-timeout (for WS disconnect, not session close) */
  drainInteractive(): void {
    for (const [, p] of this.questions) p.resolve({ behavior: 'allow', updatedInput: { answers: {} } })
    this.questions.clear()
    for (const [, p] of this.plans) p.resolve({ behavior: 'deny', message: 'Frontend disconnected' })
    this.plans.clear()
    for (const [, p] of this.elicitations) p.resolve({ behavior: 'allow', updatedInput: { response: '' } })
    this.elicitations.clear()
  }

  get pendingCount(): number {
    return this.permissions.size + this.questions.size + this.plans.size + this.elicitations.size
  }
}
```

- [ ] **Step 4: Run tests**

Run: `cd sidecar && npx vitest run src/permission-handler.test.ts`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add sidecar/src/permission-handler.ts sidecar/src/permission-handler.test.ts
git commit -m "feat(sidecar): add permission-handler — canUseTool routing with full context forwarding"
```

---

### Task 4: Create `sidecar/src/session-registry.ts`

**Files:**
- Create: `sidecar/src/session-registry.ts`

- [ ] **Step 1: Write session-registry**

```typescript
// sidecar/src/session-registry.ts
import { EventEmitter } from 'node:events'
import type { SDKSession } from '@anthropic-ai/claude-agent-sdk'
import type { PermissionHandler } from './permission-handler.js'
import type { ActiveSession, SequencedEvent, ServerEvent } from './protocol.js'
import { RingBuffer } from './ring-buffer.js'

export type SessionState = 'initializing' | 'waiting_input' | 'active' | 'waiting_permission' | 'compacting' | 'error' | 'closed'

export interface ControlSession {
  controlId: string
  sessionId: string
  sdkSession: SDKSession
  state: SessionState
  totalCostUsd: number
  turnCount: number
  modelUsage: Record<string, unknown>
  startedAt: number
  emitter: EventEmitter
  eventBuffer: RingBuffer<{ seq: number; msg: SequencedEvent }>
  nextSeq: number
  permissions: PermissionHandler
}

export class SessionRegistry {
  private sessions = new Map<string, ControlSession>()

  get(controlId: string): ControlSession | undefined {
    return this.sessions.get(controlId)
  }

  getBySessionId(sessionId: string): ControlSession | undefined {
    for (const cs of this.sessions.values()) {
      if (cs.sessionId === sessionId) return cs
    }
    return undefined
  }

  hasSessionId(sessionId: string): boolean {
    return this.getBySessionId(sessionId) !== undefined
  }

  register(cs: ControlSession): void {
    this.sessions.set(cs.controlId, cs)
  }

  remove(controlId: string): void {
    this.sessions.delete(controlId)
  }

  list(): ActiveSession[] {
    return Array.from(this.sessions.values()).map((cs) => ({
      controlId: cs.controlId,
      sessionId: cs.sessionId,
      state: cs.state,
      turnCount: cs.turnCount,
      totalCostUsd: cs.totalCostUsd || null,
      startedAt: cs.startedAt,
    }))
  }

  get activeCount(): number {
    return this.sessions.size
  }

  emitSequenced(cs: ControlSession, event: ServerEvent): void {
    const seq = cs.nextSeq++
    const sequenced: SequencedEvent = { ...event, seq }
    cs.eventBuffer.push({ seq, msg: sequenced })
    cs.emitter.emit('message', sequenced)
  }

  async closeAll(): Promise<void> {
    for (const cs of this.sessions.values()) {
      cs.permissions.drainAll()
      cs.sdkSession.close()
    }
    this.sessions.clear()
  }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd sidecar && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 3: Commit**

```bash
git add sidecar/src/session-registry.ts
git commit -m "feat(sidecar): add session-registry — typed session map with sequenced event emission"
```

---

### Task 5: Create `sidecar/src/sdk-session.ts`

The core session management — create, resume, stream loop, send.

**Files:**
- Create: `sidecar/src/sdk-session.ts`

- [ ] **Step 1: Write SDK session module**

```typescript
// sidecar/src/sdk-session.ts
import { EventEmitter } from 'node:events'
import {
  type SDKSession,
  listSessions,
  unstable_v2_createSession,
  unstable_v2_resumeSession,
} from '@anthropic-ai/claude-agent-sdk'
import { mapSdkMessage } from './event-mapper.js'
import { PermissionHandler } from './permission-handler.js'
import type {
  AvailableSession,
  CreateSessionRequest,
  ResumeSessionRequest,
  ServerEvent,
} from './protocol.js'
import { RingBuffer } from './ring-buffer.js'
import { type ControlSession, type SessionRegistry, type SessionState } from './session-registry.js'

function buildSdkOptions(
  opts: { model: string; permissionMode?: string; allowedTools?: string[]; disallowedTools?: string[] },
  permissions: PermissionHandler,
  emitFn: (event: ServerEvent) => void,
) {
  return {
    model: opts.model,
    ...(opts.permissionMode ? { permissionMode: opts.permissionMode as any } : {}),
    ...(opts.allowedTools ? { allowedTools: opts.allowedTools } : {}),
    ...(opts.disallowedTools ? { disallowedTools: opts.disallowedTools } : {}),
    canUseTool: async (
      toolName: string,
      input: Record<string, unknown>,
      toolOpts: { signal: AbortSignal; suggestions?: unknown[]; blockedPath?: string; decisionReason?: string; toolUseID: string; agentID?: string },
    ) => {
      return permissions.handleCanUseTool(toolName, input, toolOpts, emitFn)
    },
  }
}

export async function createControlSession(
  req: CreateSessionRequest,
  registry: SessionRegistry,
): Promise<ControlSession> {
  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()

  const cs: ControlSession = {
    controlId,
    sessionId: '', // filled after first message from SDK
    sdkSession: null as unknown as SDKSession,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
  }

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const sdkSession = unstable_v2_createSession(
    buildSdkOptions(
      { model: req.model, permissionMode: req.permissionMode, allowedTools: req.allowedTools, disallowedTools: req.disallowedTools },
      permissions,
      emit,
    ),
  )

  cs.sdkSession = sdkSession
  registry.register(cs)

  // Start stream loop
  runStreamLoop(cs, registry)

  // Send initial message if provided
  if (req.initialMessage) {
    await sdkSession.send(req.initialMessage)
  }

  return cs
}

export async function resumeControlSession(
  req: ResumeSessionRequest,
  registry: SessionRegistry,
): Promise<ControlSession> {
  // Check if already active
  const existing = registry.getBySessionId(req.sessionId)
  if (existing) return existing

  const controlId = crypto.randomUUID()
  const emitter = new EventEmitter()
  const permissions = new PermissionHandler()

  const cs: ControlSession = {
    controlId,
    sessionId: req.sessionId,
    sdkSession: null as unknown as SDKSession,
    state: 'initializing',
    totalCostUsd: 0,
    turnCount: 0,
    modelUsage: {},
    startedAt: Date.now(),
    emitter,
    eventBuffer: new RingBuffer(200),
    nextSeq: 0,
    permissions,
  }

  const emit = (event: ServerEvent) => registry.emitSequenced(cs, event)

  const sdkSession = unstable_v2_resumeSession(
    req.sessionId,
    buildSdkOptions(
      { model: req.model ?? 'claude-sonnet-4-20250514', permissionMode: req.permissionMode },
      permissions,
      emit,
    ),
  )

  cs.sdkSession = sdkSession
  registry.register(cs)

  // Start long-lived stream loop
  runStreamLoop(cs, registry)

  return cs
}

/** One long-lived stream loop per session. Runs until session ends. */
function runStreamLoop(cs: ControlSession, registry: SessionRegistry): void {
  ;(async () => {
    try {
      for await (const msg of cs.sdkSession.stream()) {
        const events = mapSdkMessage(msg)
        for (const event of events) {
          // Update session state from certain events
          updateSessionState(cs, event)
          registry.emitSequenced(cs, event)
        }
      }
      // Stream ended normally
      cs.state = 'closed'
      registry.emitSequenced(cs, { type: 'session_closed', reason: 'stream_ended' })
    } catch (err) {
      cs.state = 'error'
      registry.emitSequenced(cs, {
        type: 'error',
        message: err instanceof Error ? err.message : String(err),
        fatal: true,
      })
    }
  })()
}

/** Update ControlSession state from protocol events */
function updateSessionState(cs: ControlSession, event: ServerEvent): void {
  switch (event.type) {
    case 'session_init':
      cs.state = 'waiting_input'
      // Capture sessionId from SDK if not set (create flow)
      if (!cs.sessionId) {
        try { cs.sessionId = cs.sdkSession.sessionId } catch { /* not ready yet */ }
      }
      break
    case 'assistant_text':
    case 'assistant_thinking':
    case 'tool_use_start':
      cs.state = 'active'
      break
    case 'turn_complete':
      cs.state = 'waiting_input'
      cs.totalCostUsd = event.totalCostUsd
      cs.turnCount = event.numTurns
      cs.modelUsage = event.modelUsage
      break
    case 'turn_error':
      cs.state = 'waiting_input' // allow retry
      cs.totalCostUsd = event.totalCostUsd
      cs.turnCount = event.numTurns
      break
    case 'permission_request':
    case 'ask_question':
    case 'plan_approval':
    case 'elicitation':
      cs.state = 'waiting_permission'
      break
    case 'session_status':
      if (event.status === 'compacting') cs.state = 'compacting'
      else if (cs.state === 'compacting') cs.state = 'waiting_input'
      break
  }
}

export async function sendMessage(cs: ControlSession, content: string): Promise<void> {
  cs.state = 'active'
  await cs.sdkSession.send(content)
}

export async function closeSession(cs: ControlSession, registry: SessionRegistry): Promise<void> {
  cs.permissions.drainAll()
  cs.sdkSession.close()
  registry.remove(cs.controlId)
}

export async function listAvailableSessions(): Promise<AvailableSession[]> {
  const sessions = await listSessions()
  return sessions.map((s) => ({
    sessionId: s.sessionId,
    summary: s.summary,
    lastModified: s.lastModified,
    fileSize: s.fileSize,
    customTitle: s.customTitle,
    firstPrompt: s.firstPrompt,
    gitBranch: s.gitBranch,
    cwd: s.cwd,
  }))
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd sidecar && npx tsc --noEmit`
Expected: Clean (may show warnings for unused vars — acceptable at this stage)

- [ ] **Step 3: Commit**

```bash
git add sidecar/src/sdk-session.ts
git commit -m "feat(sidecar): add sdk-session — create/resume with long-lived stream loop"
```

---

## Chunk 3: Routes, WS Handler & Server Wiring

### Task 6: Create `sidecar/src/routes.ts`

**Files:**
- Create: `sidecar/src/routes.ts`

- [ ] **Step 1: Write HTTP routes**

```typescript
// sidecar/src/routes.ts
import { Hono } from 'hono'
import type {
  CreateSessionRequest,
  PromptRequest,
  ResumeSessionRequest,
} from './protocol.js'
import {
  closeSession,
  createControlSession,
  listAvailableSessions,
  resumeControlSession,
  sendMessage,
} from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

export function createRoutes(registry: SessionRegistry) {
  const app = new Hono()

  // Create new session
  app.post('/sessions', async (c) => {
    const body = await c.req.json<CreateSessionRequest>()
    if (!body.model) return c.json({ error: 'model is required' }, 400)

    try {
      const cs = await createControlSession(body, registry)
      return c.json({
        controlId: cs.controlId,
        sessionId: cs.sessionId,
        status: 'created',
      })
    } catch (err) {
      return c.json({ error: `Create failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Resume existing session
  app.post('/sessions/resume', async (c) => {
    const body = await c.req.json<ResumeSessionRequest>()
    if (!body.sessionId?.match(/^[0-9a-f-]{36}$/)) {
      return c.json({ error: 'Invalid session ID format' }, 400)
    }

    // Check if already resumed
    if (registry.hasSessionId(body.sessionId)) {
      const existing = registry.getBySessionId(body.sessionId)!
      return c.json({
        controlId: existing.controlId,
        sessionId: body.sessionId,
        status: 'already_active',
      })
    }

    try {
      const cs = await resumeControlSession(body, registry)
      return c.json({
        controlId: cs.controlId,
        sessionId: body.sessionId,
        status: 'resumed',
      })
    } catch (err) {
      return c.json({ error: `Resume failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Send message
  app.post('/send', async (c) => {
    const body = await c.req.json<{ controlId: string; message: string }>()
    const cs = registry.get(body.controlId)
    if (!cs) return c.json({ error: 'Session not found' }, 404)

    try {
      sendMessage(cs, body.message).catch((err) => {
        console.error(`[sidecar] sendMessage error: ${err}`)
      })
      return c.json({ status: 'sent' })
    } catch (err) {
      return c.json({ error: `Send failed: ${err}` }, 500)
    }
  })

  // List active control sessions
  app.get('/sessions', (c) => c.json(registry.list()))

  // List available Claude Code sessions
  app.get('/available-sessions', async (c) => {
    try {
      const sessions = await listAvailableSessions()
      return c.json(sessions)
    } catch (err) {
      return c.json({ error: `List failed: ${err instanceof Error ? err.message : err}` }, 500)
    }
  })

  // Terminate session
  app.delete('/sessions/:controlId', async (c) => {
    const controlId = c.req.param('controlId')
    const cs = registry.get(controlId)
    if (cs) await closeSession(cs, registry)
    return c.json({ status: 'terminated' })
  })

  return app
}
```

- [ ] **Step 2: Commit**

```bash
git add sidecar/src/routes.ts
git commit -m "feat(sidecar): add routes — create, resume, list, list-available, send, terminate"
```

---

### Task 7: Rewrite `sidecar/src/ws-handler.ts`

**Files:**
- Modify: `sidecar/src/ws-handler.ts`

- [ ] **Step 1: Rewrite WS handler**

```typescript
// sidecar/src/ws-handler.ts
import type { WebSocket } from 'ws'
import type { ClientMessage, ResumeMsg, ServerEvent } from './protocol.js'
import { sendMessage } from './sdk-session.js'
import type { SessionRegistry } from './session-registry.js'

export function handleWebSocket(ws: WebSocket, controlId: string, registry: SessionRegistry) {
  const session = registry.get(controlId)
  if (!session) {
    ws.send(JSON.stringify({ type: 'error', message: 'Session not found', fatal: true }))
    ws.close()
    return
  }

  // Subscribe to session events
  const onMessage = (msg: ServerEvent) => {
    if (ws.readyState === ws.OPEN) {
      ws.send(JSON.stringify(msg))
    }
  }
  session.emitter.on('message', onMessage)

  // Send current state
  registry.emitSequenced(session, {
    type: 'session_status',
    status: session.state === 'compacting' ? 'compacting' : null,
  })

  // Heartbeat config (no seq)
  ws.send(JSON.stringify({ type: 'heartbeat_config', intervalMs: 15_000 }))

  // Handle incoming messages
  ws.on('message', (raw) => {
    try {
      const msg: ClientMessage = JSON.parse(raw.toString())
      switch (msg.type) {
        case 'user_message':
          sendMessage(session, msg.content).catch((err) => {
            ws.send(JSON.stringify({ type: 'error', message: String(err), fatal: false }))
          })
          break

        case 'permission_response':
          session.permissions.resolvePermission(msg.requestId, msg.allowed, msg.updatedPermissions)
          break

        case 'question_response':
          if (!session.permissions.resolveQuestion(msg.requestId, msg.answers)) {
            ws.send(JSON.stringify({ type: 'error', message: 'Unknown question requestId', fatal: false }))
          }
          break

        case 'plan_response':
          if (!session.permissions.resolvePlan(msg.requestId, msg.approved, msg.feedback)) {
            ws.send(JSON.stringify({ type: 'error', message: 'Unknown plan requestId', fatal: false }))
          }
          break

        case 'elicitation_response':
          if (!session.permissions.resolveElicitation(msg.requestId, msg.response)) {
            ws.send(JSON.stringify({ type: 'error', message: 'Unknown elicitation requestId', fatal: false }))
          }
          break

        case 'resume': {
          const lastSeq = (msg as ResumeMsg).lastSeq
          const missed = session.eventBuffer.getAfter(lastSeq, (e) => e.seq)
          if (missed === null) {
            ws.send(JSON.stringify({ type: 'error', message: 'replay_buffer_exhausted', fatal: true }))
            ws.close()
          } else {
            for (const event of missed) {
              if (ws.readyState === ws.OPEN) ws.send(JSON.stringify(event.msg))
            }
          }
          break
        }

        case 'ping':
          ws.send(JSON.stringify({ type: 'pong' }))
          break
      }
    } catch {
      ws.send(JSON.stringify({ type: 'error', message: 'Invalid message format', fatal: false }))
    }
  })

  // Cleanup on close — drain interactive maps, keep session alive for reconnect
  ws.on('close', () => {
    session.emitter.removeListener('message', onMessage)
    session.permissions.drainInteractive()
  })
}
```

- [ ] **Step 2: Commit**

```bash
git add sidecar/src/ws-handler.ts
git commit -m "refactor(sidecar): rewrite ws-handler — delegates to PermissionHandler, handles all client messages"
```

---

### Task 8: Rewrite `sidecar/src/index.ts` and cleanup

**Files:**
- Modify: `sidecar/src/index.ts`
- Delete: `sidecar/src/types.ts`, `sidecar/src/session-manager.ts`, `sidecar/src/control.ts`, `sidecar/src/control.test.ts`

- [ ] **Step 1: Rewrite index.ts**

```typescript
// sidecar/src/index.ts
import fs from 'node:fs'
import { createAdaptorServer } from '@hono/node-server'
import { Hono } from 'hono'
import { WebSocketServer } from 'ws'
import { healthRouter } from './health.js'
import { createRoutes } from './routes.js'
import { SessionRegistry } from './session-registry.js'
import { handleWebSocket } from './ws-handler.js'

const SOCKET_PATH = process.env.SIDECAR_SOCKET ?? `/tmp/claude-view-sidecar-${process.ppid}.sock`

const registry = new SessionRegistry()
const app = new Hono()

app.route('/health', healthRouter(() => registry.activeCount))
app.route('/control', createRoutes(registry))
app.get('/', (c) => c.json({ status: 'ok' }))

// Clean up stale socket
if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)

const server = createAdaptorServer(app)
server.listen(SOCKET_PATH, () => {
  console.log(`[sidecar] Listening on ${SOCKET_PATH}`)
  console.log(`[sidecar] PID: ${process.pid}, Parent PID: ${process.ppid}`)
})

// WS upgrade
const wss = new WebSocketServer({ noServer: true })
server.on('upgrade', (request, socket, head) => {
  const match = request.url?.match(/\/control\/sessions\/([^/]+)\/stream/)
  if (!match?.[1]) { socket.destroy(); return }

  const controlId = match[1]
  wss.handleUpgrade(request, socket, head, (ws) => {
    handleWebSocket(ws, controlId, registry)
  })
})

// Parent process check
const parentCheck = setInterval(() => {
  try { process.kill(process.ppid!, 0) }
  catch { console.log('[sidecar] Parent exited, shutting down'); shutdown() }
}, 2000)

async function shutdown() {
  clearInterval(parentCheck)
  await registry.closeAll()
  server.close()
  if (fs.existsSync(SOCKET_PATH)) fs.unlinkSync(SOCKET_PATH)
  process.exit(0)
}

process.on('SIGTERM', () => void shutdown())
process.on('SIGINT', () => void shutdown())

export { app, registry, server, SOCKET_PATH }
```

- [ ] **Step 2: Delete old files**

```bash
cd sidecar && rm -f src/types.ts src/session-manager.ts src/control.ts src/control.test.ts
```

- [ ] **Step 3: Verify build**

Run: `cd sidecar && npx tsc --noEmit`
Expected: Clean (0 errors)

- [ ] **Step 4: Run all tests**

Run: `cd sidecar && npx vitest run`
Expected: All tests pass (event-mapper, permission-handler, ring-buffer)

- [ ] **Step 5: Commit**

```bash
git add -A sidecar/src/
git commit -m "refactor(sidecar): complete rewrite — delete old files, wire new modules"
```

---

## Chunk 4: Frontend Types & Hooks

### Task 9: Expand `apps/web/src/types/control.ts`

**Files:**
- Modify: `apps/web/src/types/control.ts`

- [ ] **Step 1: Expand frontend types to match new protocol**

Replace the server event types in `apps/web/src/types/control.ts` with types matching `sidecar/src/protocol.ts`. Keep `CostEstimate`, `ChatMessage`, `ChatMessageWithStatus`, `MessageStatus`, `CLOSE_CODES`, `NON_RECOVERABLE_CODES` as they are — they're frontend concerns.

Add to the `ServerMessage` union all 27 event types. Key additions:

```typescript
// New types to add (alongside existing ones that get renamed):

export interface AssistantTextMsg { type: 'assistant_text'; text: string; messageId: string; parentToolUseId: string | null }
export interface AssistantThinkingMsg { type: 'assistant_thinking'; thinking: string; messageId: string; parentToolUseId: string | null }
export interface AssistantErrorMsg { type: 'assistant_error'; error: string; messageId: string }
export interface TurnCompleteMsg { type: 'turn_complete'; totalCostUsd: number; numTurns: number; durationMs: number; durationApiMs: number; usage: Record<string, number>; modelUsage: Record<string, ModelUsageInfo>; permissionDenials: { toolName: string; toolUseId: string }[]; result: string; structuredOutput?: unknown; stopReason: string | null; fastModeState?: string }
export interface TurnErrorMsg { type: 'turn_error'; subtype: string; errors: string[]; permissionDenials: { toolName: string; toolUseId: string }[]; totalCostUsd: number; numTurns: number; durationMs: number; usage: Record<string, number>; modelUsage: Record<string, ModelUsageInfo>; stopReason: string | null; fastModeState?: string }
export interface SessionInitMsg { type: 'session_init'; tools: string[]; model: string; mcpServers: { name: string; status: string }[]; permissionMode: string; cwd: string; claudeCodeVersion: string; agents: string[]; skills: string[] }
export interface ContextCompactedMsg { type: 'context_compacted'; trigger: string; preTokens: number }
export interface RateLimitMsg { type: 'rate_limit'; status: string; resetsAt?: number; utilization?: number; rateLimitType?: string }
export interface TaskStartedMsg { type: 'task_started'; taskId: string; description: string; taskType?: string }
export interface TaskProgressMsg { type: 'task_progress'; taskId: string; description: string; lastToolName?: string; summary?: string; usage: { totalTokens: number; toolUses: number; durationMs: number } }
export interface TaskNotificationMsg { type: 'task_notification'; taskId: string; status: string; summary: string; usage?: { totalTokens: number; toolUses: number; durationMs: number } }
export interface ToolProgressMsg { type: 'tool_progress'; toolUseId: string; toolName: string; elapsedSeconds: number }
export interface ToolSummaryMsg { type: 'tool_summary'; summary: string; precedingToolUseIds: string[] }
export interface HookEventMsg { type: 'hook_event'; phase: string; hookId: string; hookName: string; hookEventName: string; outcome?: string }
export interface FilesSavedMsg { type: 'files_saved'; files: { filename: string; fileId: string }[]; failed: { filename: string; error: string }[] }
export interface PromptSuggestionMsg { type: 'prompt_suggestion'; suggestion: string }
export interface CommandOutputMsg { type: 'command_output'; content: string }
export interface SessionClosedMsg { type: 'session_closed'; reason: string }
export interface AuthStatusMsg { type: 'auth_status'; isAuthenticating: boolean; output: string[]; error?: string }
```

Update the `PermissionRequestMsg` to include new fields:

```typescript
export interface PermissionRequestMsg {
  type: 'permission_request'
  requestId: string
  toolName: string
  toolInput: Record<string, unknown>
  toolUseID: string              // NEW
  suggestions?: unknown[]        // NEW — for "always allow"
  decisionReason?: string        // NEW — why permission needed
  blockedPath?: string           // NEW — file that triggered
  agentID?: string               // NEW — subagent context
  timeoutMs: number
}
```

Update `ServerMessage` union to include all 27 types.

- [ ] **Step 2: Verify typecheck**

Run: `cd apps/web && npx tsc --noEmit`
Expected: May have errors in hooks that reference old type names — fix in next task

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/types/control.ts
git commit -m "feat(web): expand control types — 27 server events matching new sidecar protocol"
```

---

### Task 10: Update `apps/web/src/hooks/use-control-session.ts`

**Files:**
- Modify: `apps/web/src/hooks/use-control-session.ts`

- [ ] **Step 1: Expand the switch statement to handle all 27 event types**

Key changes in the `setUI` switch:
- `assistant_chunk` → `assistant_text` (rename, same logic: append to streaming content)
- `assistant_done` → handled by `turn_complete` and `turn_error`
- Add `assistant_thinking` — append to a `thinkingContent` state field
- Add `assistant_error` — set error state
- `tool_use_start` — same logic
- Add `tool_use_result` — append to messages (ACTUALLY WORKS NOW)
- Add `tool_progress` — update `activeToolProgress` map
- Add `tool_summary` — append to messages
- `turn_complete` — materialize streaming content, update cost/turns/usage/modelUsage
- `turn_error` — materialize, set error with details
- `session_init` — store tools/model/cwd/version
- `session_status` — update compacting state
- Add `context_compacted` — update contextCompaction
- Add `rate_limit` — update rateLimitStatus
- Add `task_started/task_progress/task_notification` — update activeTasks map
- Add `auth_status` — update authStatus
- Add `hook_event` — append to hookEvents
- Add `files_saved` — informational (could show toast)
- Add `prompt_suggestion` — update promptSuggestion
- Add `command_output` — append to messages
- Add `session_closed` — set status to completed

Add new state fields to `ControlSessionState`:
```typescript
sessionInit: SessionInitMsg | null
rateLimitStatus: RateLimitMsg | null
activeTasks: Map<string, TaskStartedMsg | TaskProgressMsg>
activeToolProgress: Map<string, ToolProgressMsg>
contextCompaction: ContextCompactedMsg | null
fastModeState: string | null
hookEvents: HookEventMsg[]
promptSuggestion: string | null
modelUsage: Record<string, ModelUsageInfo>
thinkingContent: string
```

- [ ] **Step 2: Update the reconnect WS handler similarly** (same switch, same pattern)

- [ ] **Step 3: Verify typecheck**

Run: `cd apps/web && npx tsc --noEmit`
Expected: Clean

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/hooks/use-control-session.ts
git commit -m "feat(web): expand use-control-session — handle all 27 protocol events"
```

---

### Task 11: Create `apps/web/src/hooks/use-available-sessions.ts`

**Files:**
- Create: `apps/web/src/hooks/use-available-sessions.ts`

- [ ] **Step 1: Write the hook**

```typescript
// apps/web/src/hooks/use-available-sessions.ts
import { useCallback, useEffect, useState } from 'react'

export interface AvailableSession {
  sessionId: string
  summary: string
  lastModified: number
  fileSize: number
  customTitle?: string
  firstPrompt?: string
  gitBranch?: string
  cwd?: string
}

export function useAvailableSessions() {
  const [sessions, setSessions] = useState<AvailableSession[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const res = await fetch('/api/control/available-sessions')
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const data = await res.json()
      setSessions(data)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { refresh() }, [refresh])

  return { sessions, loading, error, refresh }
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/hooks/use-available-sessions.ts
git commit -m "feat(web): add use-available-sessions hook — session picker data"
```

---

## Chunk 5: Wiring & Verification

### Task 12: Update Rust proxy route for new sidecar endpoints

**Files:**
- Modify: `crates/server/src/routes/control.rs`

- [ ] **Step 1: Add route for `/api/control/available-sessions`**

The Rust proxy needs one new route to proxy `GET /control/available-sessions` to the sidecar. Add to the `router()` function:

```rust
.route("/control/available-sessions", axum::routing::get(list_available_sessions))
```

And add the handler:

```rust
async fn list_available_sessions(State(state): State<Arc<AppState>>) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "GET", "/control/available-sessions", None).await
}
```

Also add a route for `POST /control/sessions` (create):

```rust
.route("/control/sessions", axum::routing::post(create_session).get(list_sessions))
```

```rust
async fn create_session(
    State(state): State<Arc<AppState>>,
    body: String,
) -> Result<Response, ApiError> {
    proxy_to_sidecar(&state, "POST", "/control/sessions", Some(body)).await
}
```

Update `proxy_resume` to call `/control/sessions/resume` (new path) instead of `/control/resume`.

- [ ] **Step 2: Verify Rust compiles**

Run: `cargo check -p claude-view-server`
Expected: Clean

- [ ] **Step 3: Commit**

```bash
git add crates/server/src/routes/control.rs
git commit -m "feat(server): add proxy routes for create-session and available-sessions"
```

---

### Task 13: Build & typecheck everything

- [ ] **Step 1: Typecheck sidecar**

Run: `cd sidecar && npx tsc --noEmit`
Expected: 0 errors

- [ ] **Step 2: Build sidecar**

Run: `cd sidecar && bun run build`
Expected: `dist/index.js` created

- [ ] **Step 3: Run sidecar tests**

Run: `cd sidecar && npx vitest run`
Expected: All tests pass

- [ ] **Step 4: Typecheck web frontend**

Run: `cd apps/web && npx tsc --noEmit`
Expected: 0 errors (or only pre-existing errors unrelated to control types)

- [ ] **Step 5: Build web frontend**

Run: `bun run build`
Expected: Turbo build succeeds

- [ ] **Step 6: Check Rust**

Run: `cargo check -p claude-view-server`
Expected: Clean

- [ ] **Step 7: Commit any remaining fixes**

```bash
git add -A
git commit -m "fix: resolve build/typecheck issues from sidecar revamp"
```

---

### Task 14: Manual end-to-end verification

- [ ] **Step 1: Start dev stack**

Run: `bun dev`
Expected: Rust server + web frontend start. Sidecar spawns on first control connection.

- [ ] **Step 2: Open browser, navigate to a session, click "Resume"**

Verify in browser DevTools Network tab:
- WS connection established to `/api/control/connect?sessionId=...`
- `heartbeat_config` received
- `session_init` received with tools, model, cwd

- [ ] **Step 3: Send a message, verify full lifecycle**

Type a message and verify:
- `assistant_text` events arrive (streaming content appears)
- `tool_use_start` events arrive (tool cards appear)
- `tool_use_result` events arrive (tool results appear — THIS WAS BROKEN BEFORE)
- `turn_complete` arrives with real cost/usage data (not zeros)

- [ ] **Step 4: Verify permission request**

Trigger a permission-requiring action. Verify:
- `permission_request` arrives with `toolUseID`, `decisionReason`
- Clicking Allow/Deny resolves correctly

- [ ] **Step 5: Commit verification notes**

No code change — just verify everything works end-to-end.
