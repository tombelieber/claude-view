// src/lib/message-to-rich.ts
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'
import type { ActionCategory } from '../components/live/action-log/types'

/** Strip Claude Code internal command tags from content (same logic as RichPane). */
function stripCommandTags(content: string): string {
  return content
    .replace(/<command-name>[\s\S]*?<\/command-name>/g, '')
    .replace(/<command-message>[\s\S]*?<\/command-message>/g, '')
    .replace(/<command-args>[\s\S]*?<\/command-args>/g, '')
    .replace(/<local-command-stdout>[\s\S]*?<\/local-command-stdout>/g, '')
    .replace(/<system-reminder>[\s\S]*?<\/system-reminder>/g, '')
    .trim()
}

/** Convert a timestamp string to Unix seconds, or undefined. */
function parseTimestamp(ts: string | null | undefined): number | undefined {
  if (!ts) return undefined
  const ms = Date.parse(ts)
  if (!isNaN(ms) && ms > 0) return ms / 1000
  return undefined
}

/** Try to parse a string as JSON. Returns parsed value or undefined. */
function tryParseJson(str: string): unknown | undefined {
  try {
    return JSON.parse(str)
  } catch {
    return undefined
  }
}

/**
 * Convert paginated Message[] (from JSONL parser) to RichMessage[] (for RichPane).
 *
 * Mapping (lossless — all 7 JSONL types emitted):
 * - user → user
 * - assistant → thinking (if has thinking) + assistant (if has content)
 * - tool_use → tool_use (extract tool name from tool_calls[0])
 * - tool_result → tool_result
 * - system → system (with metadata)
 * - progress → progress (with metadata)
 * - summary → summary (with metadata)
 */
export function messagesToRichMessages(messages: Message[]): RichMessage[] {
  const result: RichMessage[] = []
  let lastToolCategory: ActionCategory | undefined

  for (const msg of messages) {
    const ts = parseTimestamp(msg.timestamp)

    // Emit thinking block first (if present on assistant messages)
    if (msg.thinking) {
      const thinkingContent = stripCommandTags(msg.thinking)
      if (thinkingContent) {
        result.push({ type: 'thinking', content: thinkingContent, ts })
      }
    }

    switch (msg.role) {
      case 'user': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'user', content, ts })
        }
        break
      }

      case 'assistant': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'assistant', content, ts })
        }
        break
      }

      case 'tool_use': {
        // Emit one RichMessage per tool_call (matches WebSocket behavior).
        // Each ToolCall now carries its own input data from the Rust parser.
        const toolCalls = msg.tool_calls ?? []
        if (toolCalls.length === 0) {
          // Fallback: legacy data without individual tool calls
          const inputStr = msg.content || ''
          const category = (msg.category as ActionCategory) ?? 'builtin'
          result.push({
            type: 'tool_use',
            content: '',
            name: 'tool',
            input: inputStr || undefined,
            inputData: inputStr ? tryParseJson(inputStr) : undefined,
            ts,
            category,
          })
          lastToolCategory = category
        } else {
          for (const tc of toolCalls) {
            const inputData = tc.input ?? undefined
            const inputStr = inputData ? JSON.stringify(inputData, null, 2) : undefined
            const category = (tc.category as ActionCategory) ?? 'builtin'
            result.push({
              type: 'tool_use',
              content: '',
              name: tc.name,
              input: inputStr,
              inputData,
              ts,
              category,
            })
            lastToolCategory = category
          }
        }
        break
      }

      case 'tool_result': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'tool_result', content, ts, category: lastToolCategory })
        }
        break
      }

      case 'system': {
        const content = stripCommandTags(msg.content)
        result.push({
          type: 'system',
          content: content || '',
          ts,
          category: (msg.category as ActionCategory) ?? undefined,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      case 'progress': {
        const content = stripCommandTags(msg.content)
        result.push({
          type: 'progress',
          content: content || '',
          ts,
          category: (msg.category as ActionCategory) ?? undefined,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      case 'summary': {
        result.push({
          type: 'summary',
          content: msg.content || '',
          ts,
          metadata: msg.metadata ?? undefined,
        })
        break
      }

      default:
        break
    }
  }

  return result
}
