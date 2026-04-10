import { describe, expect, test } from 'bun:test'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

const GEN_DIR = join(import.meta.dir, '..', '..', 'src', 'tools', 'generated')

describe('codegen output', () => {
  test('generated sessions.ts exists', () => {
    expect(existsSync(join(GEN_DIR, 'sessions.ts'))).toBe(true)
  })

  test('generated stats.ts exists', () => {
    expect(existsSync(join(GEN_DIR, 'stats.ts'))).toBe(true)
  })

  test('generated live.ts exists', () => {
    expect(existsSync(join(GEN_DIR, 'live.ts'))).toBe(true)
  })

  test('generated index.ts re-exports sessions, stats, live', () => {
    const index = readFileSync(join(GEN_DIR, 'index.ts'), 'utf-8')
    expect(index).toContain('sessionsGeneratedTools')
    expect(index).toContain('statsGeneratedTools')
    expect(index).toContain('liveGeneratedTools')
  })
})

describe('no duplicate tool names across generated + hand-written', () => {
  test('all tool names are globally unique', async () => {
    const { allGeneratedTools } = await import('../../src/tools/generated/index.js')
    const { sessionTools } = await import('../../src/tools/sessions.js')
    const { statsTools } = await import('../../src/tools/stats.js')
    const { liveTools } = await import('../../src/tools/live.js')

    const allNames = [
      ...allGeneratedTools.map((t: { name: string }) => t.name),
      ...sessionTools.map((t: { name: string }) => t.name),
      ...statsTools.map((t: { name: string }) => t.name),
      ...liveTools.map((t: { name: string }) => t.name),
    ]
    const duplicates = allNames.filter((n, i) => allNames.indexOf(n) !== i)
    expect(duplicates).toEqual([])
  })
})

describe('path collision safety', () => {
  test('hand-written endpoint paths are not in generated tools', async () => {
    const { allGeneratedTools } = await import('../../src/tools/generated/index.js')

    // These are the paths served by hand-written tools
    const handWrittenPaths = new Set([
      '/api/sessions',
      '/api/sessions/{id}',
      '/api/search',
      '/api/stats/dashboard',
      '/api/score',
      '/api/stats/tokens',
      '/api/live/sessions',
      '/api/live/summary',
    ])

    // Generated tool handlers call client.request with the path
    // We check that no generated tool's handler source includes these exact paths
    for (const tool of allGeneratedTools) {
      const handlerStr = tool.handler.toString()
      for (const path of handWrittenPaths) {
        // Only match exact path in request call, not subpaths like /api/sessions/{id}/messages
        const exactPathPattern = new RegExp(`['"\`]${path.replace(/[{}]/g, '\\$&')}['"\`]`)
        expect(
          exactPathPattern.test(handlerStr),
          `Generated tool '${tool.name}' uses hand-written path ${path}`,
        ).toBe(false)
      }
    }
  })
})
