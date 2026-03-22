import { describe, expect, it } from 'vitest'

const readSource = async (relPath: string) => {
  const fs = await import('node:fs/promises')
  const path = await import('node:path')
  return fs.readFile(path.resolve(import.meta.dirname, relPath), 'utf-8')
}

// Static source analysis — verify ChatPageV2 passes liveProjectPath through dockview params
describe('ChatPageV2 wiring (regression: liveProjectPath must reach ChatSession)', () => {
  it('ChatPageV2 passes liveProjectPath in makeSessionPanelArgs', async () => {
    const source = await readSource('ChatPageV2.tsx')
    expect(source).toMatch(/liveProjectPath/)
    expect(source).toMatch(/<SessionSidebar/)
  })

  it('ChatPageV2 updates liveProjectPath on SSE tick', async () => {
    const source = await readSource('ChatPageV2.tsx')
    expect(source).toMatch(/updateParameters[\s\S]*liveProjectPath/)
  })

  it('ChatSession accepts sessionId prop (not useParams)', async () => {
    const source = await readSource('ChatSession.tsx')
    expect(source).not.toMatch(/useParams.*sessionId/)
    expect(source).toMatch(/sessionId:\s*string\s*\|\s*undefined/)
  })

  it('ChatSession accepts liveProjectPath prop', async () => {
    const source = await readSource('ChatSession.tsx')
    expect(source).toMatch(/liveProjectPath/)
  })
})
