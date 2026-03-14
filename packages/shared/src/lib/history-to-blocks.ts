import type {
  AssistantBlock,
  ConversationBlock,
  SystemBlock,
  ToolExecution,
  UserBlock,
} from '../types/blocks'
import type { UnknownSdkEvent } from '../types/sidecar-protocol'

// Local interface matching the generated Message type from Rust (apps/web/src/types/generated/Message.ts)
// Defined locally to avoid coupling shared to the web app's generated types.
interface HistoricalMessage {
  role: 'user' | 'assistant' | 'tool_use' | 'tool_result' | 'system' | 'progress'
  content: string
  uuid?: string | null
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
        const block: SystemBlock = {
          type: 'system',
          id: genId(msg.uuid),
          variant: 'unknown',
          data: { sdkType: subtype } as UnknownSdkEvent,
          rawJson: msg.raw_json as Record<string, unknown> | undefined,
        }
        blocks.push(block)
        break
      }

      case 'progress': {
        const block: SystemBlock = {
          type: 'system',
          id: genId(msg.uuid),
          variant: 'unknown',
          data: { sdkType: 'progress' } as UnknownSdkEvent,
          rawJson: msg.raw_json as Record<string, unknown> | undefined,
        }
        blocks.push(block)
        break
      }
    }
  }

  return blocks
}
