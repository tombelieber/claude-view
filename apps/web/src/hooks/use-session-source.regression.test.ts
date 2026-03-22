import { describe, expect, it } from 'vitest'

const readSource = async (relPath: string) => {
  const fs = await import('node:fs/promises')
  const path = await import('node:path')
  // Resolve relative to this test file's directory (apps/web/src/hooks/)
  return fs.readFile(
    path.resolve(import.meta.dirname, '..', relPath.replace(/^src\//, '')),
    'utf-8',
  )
}

// Root Cause 1: Missing state resets on session switch
describe('Root cause 1 regression: session switch state cleanup', () => {
  it('ChatSession does NOT use useParams — sessionId comes from props', async () => {
    const source = await readSource('src/pages/ChatSession.tsx')
    expect(source).not.toMatch(/useParams.*sessionId/)
    expect(source).toMatch(/sessionId:\s*string\s*\|\s*undefined/)
  })
})

// Root Cause 2: replayComplete is semantically wrong
describe('Root cause 2 regression: replayComplete removed from lifecycle', () => {
  it('SessionSourceResult has NO replayComplete field', async () => {
    const source = await readSource('src/hooks/use-session-source.ts')
    expect(source).not.toMatch(/replayComplete:\s*boolean/)
  })

  it('use-conversation has NO Cases 1/2/3 merge logic', async () => {
    const source = await readSource('src/hooks/use-conversation.ts')
    expect(source).not.toMatch(/replayComplete/)
    expect(source).not.toMatch(/RESUMED_DIVIDER/)
  })

  it('MessageState with committedBlocks + pendingText replaces turnVersion/streamGap', async () => {
    const source = await readSource('src/hooks/use-session-source.ts')
    expect(source).toMatch(/committedBlocks:\s*ConversationBlock\[\]/)
    expect(source).toMatch(/pendingText:\s*string/)
    // turnVersion and streamGap are removed — binary source switch replaces them
    expect(source).not.toMatch(/turnVersion:\s*number/)
    expect(source).not.toMatch(/streamGap:\s*boolean/)
  })
})

// Root Cause 3: Sidebar uses SSE not polling
describe('Root cause 3 regression: sidebar uses SSE not polling', () => {
  it('SessionSidebar has NO setInterval', async () => {
    const source = await readSource('src/components/conversation/sidebar/SessionSidebar.tsx')
    expect(source).not.toMatch(/setInterval/)
  })

  it('SessionSidebar accepts liveSessions prop', async () => {
    const source = await readSource('src/components/conversation/sidebar/SessionSidebar.tsx')
    expect(source).toMatch(/liveSessions:\s*LiveSession\[\]/)
  })

  it('ChatPageV2 passes liveSessions from useOutletContext to SessionSidebar', async () => {
    const source = await readSource('src/pages/ChatPageV2.tsx')
    expect(source).toMatch(/useOutletContext/)
    expect(source).toMatch(/liveSessions/)
    expect(source).toMatch(/<SessionSidebar\s+liveSessions=/)
  })
})
