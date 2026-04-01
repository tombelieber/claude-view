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

// ── Pagination regression: stable useCallback, unconditional pass ────
// Root cause (a888a56f): inline anonymous functions + conditional ternary
// for onStartReached broke Virtuoso's startReached binding. All consumers
// MUST use a stable useCallback passed unconditionally.

describe('Pagination wiring parity (regression: a888a56f)', () => {
  const consumers = [
    { name: 'ChatSession', path: '../pages/ChatSession.tsx' },
    { name: 'ConversationView', path: 'ConversationView.tsx' },
    { name: 'SessionDetailPanel', path: 'live/SessionDetailPanel.tsx' },
  ]

  for (const { name, path } of consumers) {
    it(`${name}: uses useCallback for handleLoadOlderHistory`, async () => {
      const src = await readSource(path)
      expect(src).toMatch(/const handleLoadOlderHistory = useCallback\(/)
    })

    it(`${name}: passes onStartReached unconditionally (no ternary)`, async () => {
      const src = await readSource(path)
      // Must have onStartReached={handleLoadOlderHistory} — NOT a ternary
      expect(src).toMatch(/onStartReached=\{handleLoadOlderHistory\}/)
    })

    it(`${name}: never uses inline arrow for onStartReached`, async () => {
      const src = await readSource(path)
      // Negative: no inline dispatch in onStartReached prop
      expect(src).not.toMatch(/onStartReached=\{[^}]*=>\s*(?:conv)?[Dd]ispatch/)
    })
  }
})

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
