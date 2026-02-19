// src/lib/message-to-rich.ts
import type { Message } from '../types/generated'
import type { RichMessage } from '../components/live/RichPane'

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
 * Mapping:
 * - user → user
 * - assistant → thinking (if has thinking) + assistant (if has content)
 * - tool_use → tool_use (extract tool name from tool_calls[0])
 * - tool_result → tool_result
 * - system → assistant (rendered as info)
 * - progress → skipped
 * - summary → skipped
 */
export function messagesToRichMessages(messages: Message[]): RichMessage[] {
  const result: RichMessage[] = []

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
        const toolName = msg.tool_calls?.[0]?.name ?? 'tool'
        const inputStr = msg.content || ''
        result.push({
          type: 'tool_use',
          content: '',
          name: toolName,
          input: inputStr || undefined,
          inputData: inputStr ? tryParseJson(inputStr) : undefined,
          ts,
        })
        break
      }

      case 'tool_result': {
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'tool_result', content, ts })
        }
        break
      }

      case 'system': {
        // Render system messages as assistant info
        const content = stripCommandTags(msg.content)
        if (content) {
          result.push({ type: 'assistant', content, ts })
        }
        break
      }

      // progress, summary → skip (not useful in replay)
      default:
        break
    }
  }

  return result
}
