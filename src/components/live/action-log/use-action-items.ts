import { useMemo } from 'react'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator, TimelineItem, ActionCategory, HookEventItem } from './types'

function categorize(toolName: string): ActionCategory {
  if (toolName === 'Skill') return 'skill'
  if (toolName.startsWith('mcp__') || toolName.startsWith('mcp_')) return 'mcp'
  if (toolName === 'Task') return 'agent'
  return 'builtin'
}

function makeLabel(toolName: string, input?: string): string {
  if (!input) return toolName

  try {
    const parsed = JSON.parse(input)

    // File operations
    if (toolName === 'Edit' || toolName === 'Write' || toolName === 'Read') {
      const fp = parsed.file_path || parsed.path || ''
      const parts = fp.split('/')
      const short = parts.length > 2 ? `.../${parts.slice(-2).join('/')}` : fp
      return `${toolName} ${short}`
    }

    // Bash
    if (toolName === 'Bash') {
      const cmd = parsed.command || parsed.cmd || ''
      const firstLine = cmd.split('\n')[0]
      return firstLine.length > 60 ? firstLine.slice(0, 57) + '...' : firstLine
    }

    // Search
    if (toolName === 'Grep') {
      return `Grep "${parsed.pattern || ''}"`
    }
    if (toolName === 'Glob') {
      return `Glob ${parsed.pattern || ''}`
    }

    // Skill
    if (toolName === 'Skill') {
      return `Skill: ${parsed.skill || parsed.name || 'unknown'}`
    }

    // Task (sub-agent)
    if (toolName === 'Task') {
      const desc = parsed.description || parsed.prompt || ''
      return desc.length > 50 ? `Task: ${desc.slice(0, 47)}...` : `Task: ${desc}`
    }

    // MCP tools
    if (toolName.startsWith('mcp__')) {
      const parts = toolName.split('__')
      const shortName = parts.length >= 3 ? `${parts[1]}:${parts[2]}` : toolName
      return shortName
    }

    return toolName
  } catch {
    return toolName
  }
}

export function useActionItems(messages: RichMessage[], hookEvents?: HookEventItem[]): TimelineItem[] {
  const hookEventsLength = hookEvents?.length ?? 0

  return useMemo(() => {
    const items: TimelineItem[] = []
    let actionIndex = 0
    const pendingToolUses: ActionItem[] = []

    for (const msg of messages) {
      // Turn separators for user/assistant messages
      if (msg.type === 'user' || msg.type === 'assistant') {
        const text = msg.content.trim()
        if (text) {
          items.push({
            id: `turn-${items.length}`,
            type: 'turn',
            role: msg.type,
            content: text.length > 100 ? text.slice(0, 97) + '...' : text,
            timestamp: msg.ts,
          } satisfies TurnSeparator)
        }
        continue
      }

      // Tool use -> create pending action
      if (msg.type === 'tool_use' && msg.name) {
        const action: ActionItem = {
          id: `action-${actionIndex++}`,
          timestamp: msg.ts,
          category: categorize(msg.name),
          toolName: msg.name,
          label: makeLabel(msg.name, msg.input),
          status: 'pending',
          input: msg.input,
        }
        items.push(action)
        pendingToolUses.push(action)
        continue
      }

      // Tool result -> pair with most recent pending tool_use
      if (msg.type === 'tool_result') {
        const pending = pendingToolUses.pop()
        if (pending) {
          pending.output = msg.content
          if (pending.timestamp && msg.ts) {
            pending.duration = Math.round((msg.ts - pending.timestamp) * 1000)
          }
          const isError = msg.content.startsWith('Error:') ||
                          msg.content.startsWith('FAILED') ||
                          msg.content.includes('exit code') ||
                          msg.content.includes('Command failed')
          pending.status = isError ? 'error' : 'success'
        }
        continue
      }

      // Errors
      if (msg.type === 'error') {
        items.push({
          id: `action-${actionIndex++}`,
          timestamp: msg.ts,
          category: 'error',
          toolName: 'Error',
          label: msg.content.length > 60 ? msg.content.slice(0, 57) + '...' : msg.content,
          status: 'error',
          output: msg.content,
        } satisfies ActionItem)
      }
    }

    // Merge hook events into timeline
    if (hookEvents && hookEvents.length > 0) {
      for (const event of hookEvents) {
        items.push(event)
      }
      // Re-sort all items by timestamp
      items.sort((a, b) => {
        const tsA = 'timestamp' in a ? (a.timestamp ?? 0) : 0
        const tsB = 'timestamp' in b ? (b.timestamp ?? 0) : 0
        return tsA - tsB
      })
    }

    return items
  }, [messages, hookEventsLength])
}
