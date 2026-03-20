import { describe, expect, it } from 'vitest'

// Static source analysis — verify key={sessionId} is present in ChatPage
describe('ChatPage key={sessionId} wiring (regression: stale state on session switch)', () => {
  it('ChatPage renders ChatSession with key prop derived from sessionId', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(path.resolve(process.cwd(), 'src/pages/ChatPage.tsx'), 'utf-8')
    // key={sessionId ?? 'new'} must be present
    expect(source).toMatch(/key=\{sessionId\s*\?\?\s*['"]new['"]\}/)
    // SessionSidebar must be rendered as sibling (not inside ChatSession)
    expect(source).toMatch(/<SessionSidebar/)
    // ChatSession must be imported and used
    expect(source).toMatch(/import.*ChatSession.*from/)
    expect(source).toMatch(/<ChatSession/)
  })

  it('ChatSession accepts sessionId prop (not useParams)', async () => {
    const fs = await import('node:fs/promises')
    const path = await import('node:path')
    const source = await fs.readFile(
      path.resolve(process.cwd(), 'src/pages/ChatSession.tsx'),
      'utf-8',
    )
    // Must NOT use useParams for sessionId
    expect(source).not.toMatch(/useParams.*sessionId/)
    // Must accept sessionId as prop
    expect(source).toMatch(/sessionId:\s*string\s*\|\s*undefined/)
  })
})
