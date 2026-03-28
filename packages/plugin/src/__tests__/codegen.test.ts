import { describe, expect, test } from 'bun:test'

describe('codegen', () => {
  test('generated index exports tools', async () => {
    const mod = await import('../tools/generated/index.js')
    expect(mod.allGeneratedTools).toBeDefined()
    expect(Array.isArray(mod.allGeneratedTools)).toBe(true)
    expect(mod.allGeneratedTools.length).toBeGreaterThan(50)
  })

  test('each tool has required fields', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    for (const tool of allGeneratedTools) {
      expect(tool.name).toBeDefined()
      expect(tool.description).toBeDefined()
      expect(tool.inputSchema).toBeDefined()
      expect(tool.annotations).toBeDefined()
      expect(tool.handler).toBeDefined()
      expect(typeof tool.handler).toBe('function')
    }
  })

  test('tool names are unique', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    const names = allGeneratedTools.map((t) => t.name)
    expect(new Set(names).size).toBe(names.length)
  })

  test('no tool name conflicts with hand-written tools', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    const { sessionTools } = await import('../tools/sessions.js')
    const { statsTools } = await import('../tools/stats.js')
    const { liveTools } = await import('../tools/live.js')

    const handWritten = new Set(
      [...sessionTools, ...statsTools, ...liveTools].map((t) => t.name),
    )
    const generated = allGeneratedTools.map((t) => t.name)
    const conflicts = generated.filter((n) => handWritten.has(n))
    expect(conflicts).toEqual([])
  })

  test('tool names are valid snake_case identifiers', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    for (const tool of allGeneratedTools) {
      expect(tool.name).toMatch(/^[a-z][a-z0-9_]*$/)
    }
  })

  test('annotations have correct shape', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    for (const tool of allGeneratedTools) {
      expect(typeof tool.annotations.readOnlyHint).toBe('boolean')
      expect(typeof tool.annotations.destructiveHint).toBe('boolean')
      expect(typeof tool.annotations.openWorldHint).toBe('boolean')
    }
  })

  test('descriptions do not contain newlines', async () => {
    const { allGeneratedTools } = await import('../tools/generated/index.js')
    for (const tool of allGeneratedTools) {
      expect(tool.description).not.toContain('\n')
    }
  })
})
