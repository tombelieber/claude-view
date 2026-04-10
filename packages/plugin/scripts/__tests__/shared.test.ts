import { describe, expect, test } from 'bun:test'
import { HAND_WRITTEN_TAGS, SKIP_OPERATION_IDS, makeToolName } from '../shared.js'

describe('HAND_WRITTEN_TAGS', () => {
  test('is empty — tag-level blocking replaced by SKIP_OPERATION_IDS', () => {
    expect(HAND_WRITTEN_TAGS.size).toBe(0)
  })
})

describe('SKIP_OPERATION_IDS', () => {
  test('contains all hand-written session tool operationIds', () => {
    expect(SKIP_OPERATION_IDS.has('list_sessions')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('get_session_detail')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('search_handler')).toBe(true)
  })

  test('contains all hand-written stats tool operationIds', () => {
    expect(SKIP_OPERATION_IDS.has('dashboard_stats')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('stats_tokens')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('get_fluency_score')).toBe(true)
  })

  test('contains all hand-written live tool operationIds', () => {
    expect(SKIP_OPERATION_IDS.has('list_live_sessions')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('get_live_summary')).toBe(true)
  })

  test('contains large-payload operationIds', () => {
    expect(SKIP_OPERATION_IDS.has('get_session_parsed')).toBe(true)
    expect(SKIP_OPERATION_IDS.has('get_session_rich')).toBe(true)
  })

  test('contains internal/sidecar control operationIds', () => {
    for (const id of [
      'handle_hook',
      'handle_statusline',
      'bind_control',
      'unbind_control',
      'kill_session',
      'get_session_statusline_debug',
    ]) {
      expect(SKIP_OPERATION_IDS.has(id)).toBe(true)
    }
  })

  test('has exactly 16 entries', () => {
    expect(SKIP_OPERATION_IDS.size).toBe(16)
  })

  test('no overlap between SKIP_OPERATION_IDS and generated tool names', async () => {
    const { allGeneratedTools } = await import('../../src/tools/generated/index.js')
    // Generated tool names are tag_operationId, but operationIds in SKIP should
    // not appear as any generated tool's underlying operationId
    const generatedNames = new Set(allGeneratedTools.map((t: { name: string }) => t.name))
    for (const skipId of SKIP_OPERATION_IDS) {
      // The raw operationId should not be a generated tool name
      expect(generatedNames.has(skipId)).toBe(false)
    }
  })
})

describe('makeToolName', () => {
  test('avoids stutter when operationId starts with tag', () => {
    expect(makeToolName('sessions', 'sessions_list')).toBe('sessions_list')
  })

  test('prefixes tag when operationId does not start with tag', () => {
    expect(makeToolName('sessions', 'get_file_history')).toBe('sessions_get_file_history')
  })
})
