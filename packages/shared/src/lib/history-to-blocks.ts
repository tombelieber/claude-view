import type {
  AssistantBlock,
  ConversationBlock,
  ProgressBlock,
  SystemBlock,
  ToolExecution,
  UserBlock,
} from '../types/blocks'

// Local interface matching the generated Message type from Rust (apps/web/src/types/generated/Message.ts)
// Defined locally to avoid coupling shared to the web app's generated types.
interface HistoricalMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress'
  content: string
  uuid?: string | null
  parent_uuid?: string | null
  thinking?: string | null
  tool_calls?: Array<{
    name: string
    count: number
    input?: unknown
    category?: string | null
  }> | null
  timestamp?: string | null
  metadata?: unknown
  category?: string | null
  raw_json?: unknown
}

let _counter = 0
function genId(uuid?: string | null): string {
  return uuid ?? `hist-block-${++_counter}`
}

/**
 * Pure function — maps historical Message[] from the Rust API to ConversationBlock[].
 * tool_use and tool_result messages are skipped; they are already embedded in
 * the assistant message's tool_calls field.
 */
export function historyToBlocks(messages: HistoricalMessage[]): ConversationBlock[] {
  const blocks: ConversationBlock[] = []

  for (const msg of messages) {
    switch (msg.role) {
      case 'user': {
        const block: UserBlock = {
          type: 'user',
          id: genId(msg.uuid),
          text: msg.content,
          timestamp: msg.timestamp ? new Date(msg.timestamp).getTime() / 1000 : 0,
          status: 'sent',
          parentUuid: msg.parent_uuid ?? undefined,
          rawJson: msg.raw_json as Record<string, unknown> | undefined,
        }
        blocks.push(block)
        break
      }

      case 'assistant': {
        const segments: AssistantBlock['segments'] = []

        if (msg.content.trim()) {
          segments.push({ kind: 'text', text: msg.content, parentToolUseId: null })
        }

        for (const tc of msg.tool_calls ?? []) {
          const execution: ToolExecution = {
            toolName: tc.name,
            toolInput: (tc.input as Record<string, unknown>) ?? {},
            toolUseId: genId(),
            status: 'complete',
          }
          segments.push({ kind: 'tool', execution })
        }

        const block: AssistantBlock = {
          type: 'assistant',
          id: genId(msg.uuid),
          segments,
          thinking: msg.thinking ?? undefined,
          streaming: false,
          timestamp: msg.timestamp ? new Date(msg.timestamp).getTime() / 1000 : undefined,
          parentUuid: msg.parent_uuid ?? undefined,
          rawJson: msg.raw_json as Record<string, unknown> | undefined,
        }
        blocks.push(block)
        break
      }

      // tool_use and tool_result are already embedded in the assistant message's
      // tool_calls field — skip them to avoid duplication.
      case 'tool_use':
      case 'tool_result':
        break

      case 'system': {
        const meta = msg.metadata as Record<string, unknown> | undefined
        const subtype = (meta?.subtype as string) ?? 'unknown'
        const variant = mapSystemSubtype(subtype)
        const block: SystemBlock = {
          type: 'system',
          id: genId(msg.uuid),
          variant,
          data: (variant === 'unknown'
            ? { sdkType: subtype }
            : { type: variant, ...(meta ?? {}) }) as SystemBlock['data'],
          rawJson: msg.raw_json as Record<string, unknown> | undefined,
        }
        blocks.push(block)
        break
      }

      case 'progress': {
        const meta = msg.metadata as Record<string, unknown> | undefined
        const progressType = (meta?.progress_type as string) ?? 'bash'
        const block: ProgressBlock = {
          type: 'progress',
          id: genId(msg.uuid),
          variant: progressType as ProgressBlock['variant'],
          category: ((meta?.category as string) ?? 'builtin') as ProgressBlock['category'],
          data: ((meta?.data as Record<string, unknown>) ?? {
            type: progressType,
          }) as ProgressBlock['data'],
          ts: msg.timestamp ? new Date(msg.timestamp).getTime() / 1000 : 0,
          parentToolUseId: meta?.parent_tool_use_id as string | undefined,
        }
        blocks.push(block)
        break
      }
    }
  }

  return blocks
}

/** Map JSONL metadata.subtype → SystemBlock variant.
 *  SDK subtypes that don't map to a known variant fall through to 'unknown'. */
const SUBTYPE_TO_VARIANT: Record<string, SystemBlock['variant']> = {
  init: 'session_init',
  status: 'session_status',
  elicitation_complete: 'elicitation_complete',
  task_started: 'task_started',
  task_progress: 'task_progress',
  task_notification: 'task_notification',
  hook_started: 'hook_event',
  hook_progress: 'hook_event',
  hook_response: 'hook_event',
  files_persisted: 'files_saved',
  local_command_output: 'command_output',
}

function mapSystemSubtype(subtype: string): SystemBlock['variant'] {
  return SUBTYPE_TO_VARIANT[subtype] ?? 'unknown'
}
