import { useMemo } from 'react'
import type { RichMessage } from '../RichPane'
import type { ActionItem, TurnSeparator, TimelineItem, ActionCategory } from './types'

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

/** Map progress subtypes to their logical category */
function progressCategory(subtype: string | undefined): ActionCategory {
  switch (subtype) {
    case 'agent_progress': return 'agent'
    case 'bash_progress': return 'builtin'
    case 'mcp_progress': return 'mcp'
    case 'hook_progress': return 'hook_progress'
    case 'hook_event': return 'hook'
    case 'waiting_for_task': return 'queue'
    default: return 'system'
  }
}

/** Build a label for a progress ActionItem */
function progressLabel(m: Record<string, any>): string {
  const subtype = m.type ?? 'progress'
  switch (subtype) {
    case 'agent_progress':
      return m.prompt ? `Agent: ${(m.prompt as string).slice(0, 50)}` : 'Agent progress'
    case 'bash_progress':
      return m.command ? `$ ${(m.command as string).split('\n')[0].slice(0, 50)}` : 'Bash progress'
    case 'mcp_progress':
      return m.server ? `${m.server}:${m.method ?? ''}` : 'MCP progress'
    case 'hook_progress':
      return m.command
        ? `${m.hookEvent || m.hookName || 'hook'} → ${m.command}`
        : (m.hookEvent || m.hookName || 'hook progress')
    case 'hook_event': {
      const he = m._hookEvent
      return he ? `${he.eventName} — ${he.label}` : 'Hook event'
    }
    case 'waiting_for_task':
      return `Waiting (pos ${m.position ?? '?'})`
    default:
      return subtype
  }
}

/**
 * Pure function: convert RichMessage[] into TimelineItem[].
 * Exported for unit testing.
 *
 * IMPORTANT: No message type is dropped. Every RichMessage produces a TimelineItem.
 * After Task 0 normalization, hook events are in messages[] as progress subtypes,
 * so no separate hookEvents parameter is needed.
 */
export function buildActionItems(messages: RichMessage[]): TimelineItem[] {
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

    // Thinking → ActionItem (NOT dropped — shown in log tab)
    if (msg.type === 'thinking') {
      const preview = msg.content.split('\n')[0] || 'thinking...'
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'system',
        toolName: 'thinking',
        label: preview.length > 60 ? preview.slice(0, 57) + '...' : preview,
        status: 'success',
        output: msg.content,
      } satisfies ActionItem)
      continue
    }

    // Tool use → create pending action
    if (msg.type === 'tool_use' && msg.name) {
      const action: ActionItem = {
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: (msg.category as ActionCategory) ?? 'builtin',
        toolName: msg.name,
        label: makeLabel(msg.name, msg.input),
        status: 'pending',
        input: msg.input,
      }
      items.push(action)
      pendingToolUses.push(action)
      continue
    }

    // Tool result → pair with most recent pending tool_use
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

    // Progress events (ALL subtypes — including hook_event and hook_progress)
    if (msg.type === 'progress') {
      const m = msg.metadata ?? {}
      const subtype = m.type as string | undefined
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: msg.category ?? progressCategory(subtype),
        toolName: subtype ?? 'progress',
        label: progressLabel(m),
        status: 'success',
        output: m.output,
      } satisfies ActionItem)
      continue
    }

    // System events
    if (msg.type === 'system') {
      const m = msg.metadata ?? {}
      const subtype = (m.type ?? m.subtype) as string | undefined
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: msg.category ?? 'system',
        toolName: subtype ?? 'system',
        label: subtype ?? 'system event',
        status: 'success',
        output: msg.content || undefined,
      } satisfies ActionItem)
      continue
    }

    // Summary events
    if (msg.type === 'summary') {
      const summary = msg.metadata?.summary || msg.content || ''
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: msg.category ?? 'system',
        toolName: 'summary',
        label: summary.length > 60 ? `Session summary (${summary.split(/\s+/).length}w)` : `Summary: ${summary}`,
        status: 'success',
        output: summary,
      } satisfies ActionItem)
      continue
    }

    // Hook messages (legacy live path — before Task 0 normalization)
    if (msg.type === 'hook') {
      items.push({
        id: `action-${actionIndex++}`,
        timestamp: msg.ts,
        category: 'hook',
        toolName: msg.name ?? 'hook',
        label: msg.content.length > 60 ? msg.content.slice(0, 57) + '...' : msg.content,
        status: 'success',
        output: msg.input,
      } satisfies ActionItem)
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

  return items
}

export function useActionItems(messages: RichMessage[]): TimelineItem[] {
  return useMemo(() => buildActionItems(messages), [messages])
}
