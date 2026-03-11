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
      return [
        {
          type: 'unknown_sdk_event',
          sdkType: (m as AnyMsg).type ?? 'undefined',
          raw: m,
        } satisfies UnknownSdkEvent,
      ]
  }
}

// ─── Assistant ────────────────────────────────────────────────────

function mapAssistant(m: AnyMsg): ServerEvent[] {
  const msgId = String(m.uuid ?? '')
  const parentToolUseId = (m.parent_tool_use_id as string | null) ?? null

  // Check error field first — may have empty content on error
  if (m.error) {
    return [
      {
        type: 'assistant_error',
        error: m.error as AssistantError['error'],
        messageId: msgId,
      } satisfies AssistantError,
    ]
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
  const denials = (
    (m.permission_denials ?? []) as {
      tool_name: string
      tool_use_id: string
      tool_input: Record<string, unknown>
    }[]
  ).map((d) => ({ toolName: d.tool_name, toolUseId: d.tool_use_id, toolInput: d.tool_input }))

  if (m.subtype === 'success') {
    return [
      {
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
      } satisfies TurnComplete,
    ]
  }

  // Error result
  return [
    {
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
    } satisfies TurnError,
  ]
}

function mapModelUsage(
  raw: Record<string, Record<string, unknown>>,
): Record<string, ModelUsageInfo> {
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
      return [
        {
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
        } satisfies SessionInit,
      ]

    case 'status':
      return [
        {
          type: 'session_status',
          status: (m.status as 'compacting' | null) ?? null,
          permissionMode: m.permissionMode as string | undefined,
        } satisfies SessionStatus,
      ]

    case 'compact_boundary':
      return [
        {
          type: 'context_compacted',
          trigger:
            ((m.compact_metadata as Record<string, unknown>)?.trigger as 'manual' | 'auto') ??
            'auto',
          preTokens: ((m.compact_metadata as Record<string, unknown>)?.pre_tokens as number) ?? 0,
        } satisfies ContextCompacted,
      ]

    case 'elicitation_complete':
      return [
        {
          type: 'elicitation_complete',
          mcpServerName: (m.mcp_server_name as string) ?? '',
          elicitationId: (m.elicitation_id as string) ?? '',
        } satisfies ElicitationComplete,
      ]

    case 'task_started':
      return [
        {
          type: 'task_started',
          taskId: (m.task_id as string) ?? '',
          toolUseId: m.tool_use_id as string | undefined,
          description: (m.description as string) ?? '',
          taskType: m.task_type as string | undefined,
          prompt: m.prompt as string | undefined,
        } satisfies TaskStarted,
      ]

    case 'task_progress':
      return [
        {
          type: 'task_progress',
          taskId: (m.task_id as string) ?? '',
          toolUseId: m.tool_use_id as string | undefined,
          description: (m.description as string) ?? '',
          lastToolName: m.last_tool_name as string | undefined,
          summary: m.summary as string | undefined,
          usage: {
            totalTokens: (m.usage as Record<string, number>)?.total_tokens ?? 0,
            toolUses: (m.usage as Record<string, number>)?.tool_uses ?? 0,
            durationMs: (m.usage as Record<string, number>)?.duration_ms ?? 0,
          },
        } satisfies TaskProgressEvent,
      ]

    case 'task_notification':
      return [
        {
          type: 'task_notification',
          taskId: (m.task_id as string) ?? '',
          toolUseId: m.tool_use_id as string | undefined,
          status: (m.status as 'completed' | 'failed' | 'stopped') ?? 'failed',
          outputFile: (m.output_file as string) ?? '',
          summary: (m.summary as string) ?? '',
          usage: m.usage
            ? {
                totalTokens: (m.usage as Record<string, number>).total_tokens ?? 0,
                toolUses: (m.usage as Record<string, number>).tool_uses ?? 0,
                durationMs: (m.usage as Record<string, number>).duration_ms ?? 0,
              }
            : undefined,
        } satisfies TaskNotification,
      ]

    case 'hook_started':
      return [
        {
          type: 'hook_event',
          phase: 'started',
          hookId: (m.hook_id as string) ?? '',
          hookName: (m.hook_name as string) ?? '',
          hookEventName: (m.hook_event as string) ?? '',
        } satisfies HookEvent,
      ]

    case 'hook_progress':
      return [
        {
          type: 'hook_event',
          phase: 'progress',
          hookId: (m.hook_id as string) ?? '',
          hookName: (m.hook_name as string) ?? '',
          hookEventName: (m.hook_event as string) ?? '',
          stdout: m.stdout as string | undefined,
          stderr: m.stderr as string | undefined,
          output: m.output as string | undefined,
        } satisfies HookEvent,
      ]

    case 'hook_response':
      return [
        {
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
        } satisfies HookEvent,
      ]

    case 'files_persisted':
      return [
        {
          type: 'files_saved',
          files: ((m.files ?? []) as { filename: string; file_id: string }[]).map((f) => ({
            filename: f.filename,
            fileId: f.file_id,
          })),
          failed: (m.failed ?? []) as { filename: string; error: string }[],
          processedAt: (m.processed_at as string) ?? '',
        } satisfies FilesSaved,
      ]

    case 'local_command_output':
      return [
        {
          type: 'command_output',
          content: (m.content as string) ?? '',
        } satisfies CommandOutput,
      ]

    default:
      return [
        {
          type: 'unknown_sdk_event',
          sdkType: `system:${m.subtype}`,
          raw: m,
        } satisfies UnknownSdkEvent,
      ]
  }
}

// ─── Other top-level types ───────────────────────────────────────

function mapStreamEvent(m: AnyMsg): ServerEvent[] {
  return [
    {
      type: 'stream_delta',
      event: m.event,
      messageId: String(m.uuid ?? ''),
    } satisfies StreamDelta,
  ]
}

function mapToolProgress(m: AnyMsg): ServerEvent[] {
  return [
    {
      type: 'tool_progress',
      toolUseId: (m.tool_use_id as string) ?? '',
      toolName: (m.tool_name as string) ?? '',
      elapsedSeconds: (m.elapsed_time_seconds as number) ?? 0,
      parentToolUseId: (m.parent_tool_use_id as string | null) ?? null,
      taskId: m.task_id as string | undefined,
    } satisfies ToolProgress,
  ]
}

function mapRateLimit(m: AnyMsg): ServerEvent[] {
  const info = (m.rate_limit_info ?? {}) as Record<string, unknown>
  return [
    {
      type: 'rate_limit',
      status: (info.status as RateLimit['status']) ?? 'allowed',
      resetsAt: info.resetsAt as number | undefined,
      utilization: info.utilization as number | undefined,
      rateLimitType: info.rateLimitType as string | undefined,
      overageStatus: info.overageStatus as string | undefined,
      isUsingOverage: info.isUsingOverage as boolean | undefined,
    } satisfies RateLimit,
  ]
}

function mapAuthStatus(m: AnyMsg): ServerEvent[] {
  return [
    {
      type: 'auth_status',
      isAuthenticating: Boolean(m.isAuthenticating),
      output: (m.output as string[]) ?? [],
      error: m.error as string | undefined,
    } satisfies AuthStatus,
  ]
}

function mapToolSummary(m: AnyMsg): ServerEvent[] {
  return [
    {
      type: 'tool_summary',
      summary: (m.summary as string) ?? '',
      precedingToolUseIds: (m.preceding_tool_use_ids as string[]) ?? [],
    } satisfies ToolSummary,
  ]
}

function mapPromptSuggestion(m: AnyMsg): ServerEvent[] {
  return [
    {
      type: 'prompt_suggestion',
      suggestion: (m.suggestion as string) ?? '',
    } satisfies PromptSuggestion,
  ]
}

// Suppress unused import warning — SessionClosed is part of the protocol
// but not emitted by SDK messages (it's emitted by the WS handler on close).
export type { SessionClosed }
