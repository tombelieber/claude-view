import { createReadStream } from 'fs'
import { access } from 'fs/promises'
import { createInterface } from 'readline'

export interface ToolCall {
  name: string
  count: number
}

export interface Message {
  role: 'user' | 'assistant'
  content: string
  timestamp?: string
  toolCalls?: ToolCall[]
}

export interface ParsedSession {
  messages: Message[]
  metadata: {
    totalMessages: number
    toolCallCount: number
  }
}

interface ContentBlock {
  type: string
  text?: string
  name?: string
}

interface JsonlEntry {
  type: string
  message?: {
    role: string
    content: string | ContentBlock[]
  }
  timestamp?: string
  isMeta?: boolean
}

/**
 * Extract text content from a user message
 * User messages can be either a plain string or an array of content blocks
 */
function extractUserContent(content: string | ContentBlock[]): string | null {
  if (typeof content === 'string') {
    // Clean up command tags if present
    return content.replace(/<command-[^>]*>[^<]*<\/command-[^>]*>/g, '').trim() || null
  }

  if (Array.isArray(content)) {
    // Find text blocks (ignore tool_result blocks)
    const textBlocks = content.filter(
      (block): block is ContentBlock & { text: string } =>
        block.type === 'text' && typeof block.text === 'string'
    )

    if (textBlocks.length > 0) {
      return textBlocks.map(block => block.text).join('\n').trim() || null
    }
  }

  return null
}

/**
 * Extract text content from an assistant message
 * Assistant messages have an array of content blocks with type 'text' or 'tool_use'
 */
function extractAssistantContent(content: ContentBlock[]): { text: string | null; tools: string[] } {
  const result = { text: null as string | null, tools: [] as string[] }

  if (!Array.isArray(content)) {
    return result
  }

  // Extract text blocks
  const textBlocks = content.filter(
    (block): block is ContentBlock & { text: string } =>
      block.type === 'text' && typeof block.text === 'string'
  )

  if (textBlocks.length > 0) {
    result.text = textBlocks.map(block => block.text).join('\n').trim() || null
  }

  // Extract tool use names
  const toolUseBlocks = content.filter(
    (block): block is ContentBlock & { name: string } =>
      block.type === 'tool_use' && typeof block.name === 'string'
  )

  result.tools = toolUseBlocks.map(block => block.name)

  return result
}

/**
 * Parse a Claude Code JSONL session file into structured messages
 * Uses streaming to handle large files efficiently
 */
export async function parseSession(filePath: string): Promise<ParsedSession> {
  // Check if file exists
  try {
    await access(filePath)
  } catch {
    throw new Error(`Session file not found: ${filePath}`)
  }

  const messages: Message[] = []
  const toolCallCounts = new Map<string, number>()
  let pendingToolCalls: string[] = []

  return new Promise((resolve, reject) => {
    const stream = createReadStream(filePath, { encoding: 'utf-8' })
    const rl = createInterface({
      input: stream,
      crlfDelay: Infinity
    })

    rl.on('line', (line) => {
      if (!line.trim()) return

      try {
        const entry = JSON.parse(line) as JsonlEntry

        // Process user messages
        if (entry.type === 'user' && entry.message?.content && !entry.isMeta) {
          const content = extractUserContent(entry.message.content as string | ContentBlock[])

          if (content) {
            // If there are pending tool calls, attach them to the previous assistant message
            if (pendingToolCalls.length > 0 && messages.length > 0) {
              const lastMessage = messages[messages.length - 1]
              if (lastMessage.role === 'assistant') {
                lastMessage.toolCalls = aggregateToolCalls(pendingToolCalls)
              }
              pendingToolCalls = []
            }

            messages.push({
              role: 'user',
              content,
              timestamp: entry.timestamp
            })
          }
        }

        // Process assistant messages
        if (entry.type === 'assistant' && entry.message?.content) {
          const { text, tools } = extractAssistantContent(entry.message.content as ContentBlock[])

          // Collect tool calls
          if (tools.length > 0) {
            pendingToolCalls.push(...tools)
            tools.forEach(tool => {
              toolCallCounts.set(tool, (toolCallCounts.get(tool) || 0) + 1)
            })
          }

          // Add text message if present
          if (text) {
            // If there are pending tool calls, attach them to this message
            const message: Message = {
              role: 'assistant',
              content: text,
              timestamp: entry.timestamp
            }

            if (pendingToolCalls.length > 0) {
              message.toolCalls = aggregateToolCalls(pendingToolCalls)
              pendingToolCalls = []
            }

            messages.push(message)
          }
        }
      } catch {
        // Skip malformed JSON lines
      }
    })

    rl.on('close', () => {
      // Handle any remaining pending tool calls
      if (pendingToolCalls.length > 0 && messages.length > 0) {
        const lastMessage = messages[messages.length - 1]
        if (lastMessage.role === 'assistant' && !lastMessage.toolCalls) {
          lastMessage.toolCalls = aggregateToolCalls(pendingToolCalls)
        }
      }

      // Calculate total tool call count
      let totalToolCalls = 0
      toolCallCounts.forEach(count => {
        totalToolCalls += count
      })

      resolve({
        messages,
        metadata: {
          totalMessages: messages.length,
          toolCallCount: totalToolCalls
        }
      })
    })

    rl.on('error', (error) => {
      reject(error)
    })

    stream.on('error', (error) => {
      reject(error)
    })
  })
}

/**
 * Aggregate tool calls by name and count
 */
function aggregateToolCalls(tools: string[]): ToolCall[] {
  const counts = new Map<string, number>()

  tools.forEach(tool => {
    counts.set(tool, (counts.get(tool) || 0) + 1)
  })

  return Array.from(counts.entries()).map(([name, count]) => ({ name, count }))
}
