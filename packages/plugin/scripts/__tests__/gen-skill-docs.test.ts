import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import {
  HAND_WRITTEN_GROUPS,
  buildToolTable,
  extractGeneratedToolGroups,
} from '../gen-skill-docs.js'
import { SKIP_OPERATION_IDS, SSE_OPERATION_IDS } from '../shared.js'

const SPEC_PATH = join(import.meta.dir, '..', 'openapi.json')
const spec = JSON.parse(readFileSync(SPEC_PATH, 'utf-8'))

describe('extractGeneratedToolGroups', () => {
  const groups = extractGeneratedToolGroups(spec)
  const allToolNames = groups.flatMap((g) => g.tools.map((t) => t.name))

  test('excludes SKIP_OPERATION_IDS from generated groups', () => {
    for (const skipId of SKIP_OPERATION_IDS) {
      // Generated tool names may be tag_operationId or just operationId
      // Neither the raw operationId nor any tool name should contain it
      const found = allToolNames.some((name) => name === skipId || name.endsWith(`_${skipId}`))
      expect(found, `SKIP operationId '${skipId}' should not appear as a generated tool`).toBe(
        false,
      )
    }
  })

  test('excludes SSE_OPERATION_IDS from generated groups', () => {
    for (const sseId of SSE_OPERATION_IDS) {
      const found = allToolNames.some((name) => name === sseId || name.endsWith(`_${sseId}`))
      expect(found, `SSE operationId '${sseId}' should not appear as a generated tool`).toBe(false)
    }
  })

  test('returns non-empty groups (openapi.json has valid endpoints)', () => {
    expect(groups.length).toBeGreaterThan(0)
    expect(allToolNames.length).toBeGreaterThan(0)
  })

  test('no phantom internal endpoints in tool names', () => {
    const phantomPatterns = [
      'handle_hook',
      'bind_control',
      'get_session_parsed',
      'get_session_rich',
      'handle_statusline',
      'kill_session',
      'get_session_statusline_debug',
      'unbind_control',
    ]
    for (const pattern of phantomPatterns) {
      const found = allToolNames.some((name) => name.includes(pattern))
      expect(found, `Internal endpoint '${pattern}' must not appear in generated tools`).toBe(false)
    }
  })
})

describe('HAND_WRITTEN_GROUPS still present in combined output', () => {
  const groups = extractGeneratedToolGroups(spec)
  const allGroups = [...HAND_WRITTEN_GROUPS, ...groups]
  const table = buildToolTable(allGroups)

  test('hand-written tool names appear in tool table', () => {
    for (const group of HAND_WRITTEN_GROUPS) {
      for (const tool of group.tools) {
        expect(
          table.includes(tool.name),
          `Hand-written tool '${tool.name}' must appear in tool table`,
        ).toBe(true)
      }
    }
  })

  test('hand-written group labels appear in tool table', () => {
    for (const group of HAND_WRITTEN_GROUPS) {
      expect(
        table.includes(group.label),
        `Hand-written group '${group.label}' must appear in tool table`,
      ).toBe(true)
    }
  })

  test('hand-written groups have sortOrder < generated groups', () => {
    const hwMaxSort = Math.max(...HAND_WRITTEN_GROUPS.map((g) => g.sortOrder))
    const genMinSort = Math.min(...groups.map((g) => g.sortOrder))
    expect(hwMaxSort).toBeLessThan(genMinSort)
  })
})
