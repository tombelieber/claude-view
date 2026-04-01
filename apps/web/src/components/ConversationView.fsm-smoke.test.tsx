import { describe, expect, it } from 'vitest'

const readSource = async (relPath: string) => {
  const fs = await import('node:fs/promises')
  const path = await import('node:path')
  return fs.readFile(path.resolve(import.meta.dirname, relPath), 'utf-8')
}

/**
 * Structural smoke tests verifying ConversationView and SessionDetailPanel
 * consume blocks from the FSM (useChatPanel) instead of the deleted TanStack chain.
 *
 * Same source-analysis pattern as ChatSession.test.tsx — verify wiring contracts
 * without rendering, so they survive dependency changes.
 */

describe('ConversationView FSM wiring', () => {
  it('imports useChatPanel, not useConversation', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toContain("from '../hooks/use-chat-panel'")
    expect(src).not.toContain("from '../hooks/use-conversation'")
  })

  it('calls useChatPanel(sessionId)', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toMatch(/useChatPanel\(sessionId/)
  })

  it('calls useCommandExecutor', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toContain('useCommandExecutor(')
  })

  it('dispatches LIVE_STATUS_CHANGED', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toContain("type: 'LIVE_STATUS_CHANGED'")
  })

  it('dispatches LOAD_OLDER_HISTORY for pagination', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toContain("type: 'LOAD_OLDER_HISTORY'")
  })

  it('dispatches FSM events for interactive actions', async () => {
    const src = await readSource('ConversationView.tsx')
    expect(src).toContain("type: 'RESPOND_PERMISSION'")
    expect(src).toContain("type: 'ANSWER_QUESTION'")
    expect(src).toContain("type: 'APPROVE_PLAN'")
    expect(src).toContain("type: 'SUBMIT_ELICITATION'")
  })
})

describe('SessionDetailPanel FSM wiring', () => {
  it('imports useChatPanel, not useConversation', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toContain("from '../../hooks/use-chat-panel'")
    expect(src).not.toContain("from '../../hooks/use-conversation'")
  })

  it('calls useChatPanel(data.id)', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toMatch(/useChatPanel\(data\.id/)
  })

  it('calls useCommandExecutor', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toContain('useCommandExecutor(')
  })

  it('dispatches LIVE_STATUS_CHANGED', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toContain("type: 'LIVE_STATUS_CHANGED'")
  })

  it('dispatches LOAD_OLDER_HISTORY for pagination', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toContain("type: 'LOAD_OLDER_HISTORY'")
  })

  it('dispatches FSM events for interactive actions', async () => {
    const src = await readSource('live/SessionDetailPanel.tsx')
    expect(src).toContain("type: 'RESPOND_PERMISSION'")
    expect(src).toContain("type: 'APPROVE_PLAN'")
  })
})
