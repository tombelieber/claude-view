import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { Message } from '../types/generated/Message'
import type { ToolCall } from '../types/generated/ToolCall'

/**
 * Converts a block timestamp (Unix seconds, see MessageTimestamp.tsx) to the
 * ISO string the legacy Message type carries. Returns null for missing or
 * zero timestamps so exports render no time instead of the epoch.
 */
function toIsoTimestamp(unixSeconds: number | null | undefined): string | null {
  if (!unixSeconds || unixSeconds <= 0) return null
  return new Date(unixSeconds * 1000).toISOString()
}

/**
 * Best-effort short text for system/notice blocks whose payload is untyped.
 * Priority: data.content → data.message → raw string data → variant name.
 */
function bestEffortText(variant: string, data: unknown): string {
  if (typeof data === 'string' && data.trim()) return data
  if (data && typeof data === 'object') {
    const d = data as Record<string, unknown>
    if (typeof d.content === 'string' && d.content.trim()) return d.content
    if (typeof d.message === 'string' && d.message.trim()) return d.message
  }
  return variant
}

/**
 * Converts ConversationBlock[] (the block-pipeline transcript from
 * useChatPanel — the only source for foreign-provider sessions like
 * "codex:…") into the legacy Message[] shape consumed by the HTML/PDF/
 * Markdown exporters.
 *
 * Pure function: user/assistant/system/notice blocks are mapped; all other
 * block types (interaction, turn_boundary, progress, team_transcript) carry
 * no exportable conversation text and are skipped.
 */
export function blocksToMessages(blocks: ConversationBlock[]): Message[] {
  const messages: Message[] = []

  for (const block of blocks) {
    switch (block.type) {
      case 'user': {
        messages.push({
          role: 'user',
          content: block.text,
          timestamp: toIsoTimestamp(block.timestamp),
          images: block.images ?? [],
        })
        break
      }
      case 'assistant': {
        const textSegments: string[] = []
        const toolCalls: ToolCall[] = []
        for (const segment of block.segments) {
          if (segment.kind === 'text') {
            textSegments.push(segment.text)
          } else if (segment.kind === 'tool') {
            toolCalls.push({
              name: segment.execution.toolName,
              count: 1,
              input: segment.execution.toolInput,
            })
          }
        }
        messages.push({
          role: 'assistant',
          content: textSegments.join('\n\n'),
          thinking: block.thinking,
          timestamp: toIsoTimestamp(block.timestamp),
          tool_calls: toolCalls,
        })
        break
      }
      case 'system':
      case 'notice': {
        messages.push({
          role: 'system',
          content: bestEffortText(block.variant, block.data),
        })
        break
      }
      default:
        // interaction / turn_boundary / progress / team_transcript: no
        // exportable conversation text — skip.
        break
    }
  }

  return messages
}
