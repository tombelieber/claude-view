import * as nodeFs from 'node:fs'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { evaluateGate } from './gate-evaluator.js'

// Spy on existsSync — works in both bun test and vitest
const existsSyncSpy = vi.spyOn(nodeFs, 'existsSync')

afterEach(() => {
  existsSyncSpy.mockReset()
})

describe('evaluateGate', () => {
  // json_field type
  it('json_field equals -- pass', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'verdict', operator: 'equals', value: 'PASS', maxRetries: 1 },
        '{"verdict":"PASS"}',
      ),
    ).toBe(true)
  })
  it('json_field equals -- fail', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'verdict', operator: 'equals', value: 'PASS', maxRetries: 1 },
        '{"verdict":"FAIL"}',
      ),
    ).toBe(false)
  })
  it('json_field contains -- pass', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'msg', operator: 'contains', value: 'ok', maxRetries: 1 },
        '{"msg":"all ok here"}',
      ),
    ).toBe(true)
  })
  it('json_field gt -- pass', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'score', operator: 'gt', value: 90, maxRetries: 1 },
        '{"score":95}',
      ),
    ).toBe(true)
  })
  it('json_field lt -- pass', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'errors', operator: 'lt', value: 5, maxRetries: 1 },
        '{"errors":2}',
      ),
    ).toBe(true)
  })
  it('json_field malformed JSON -- throws', () => {
    expect(() =>
      evaluateGate(
        { type: 'json_field', field: 'x', operator: 'equals', value: 'y', maxRetries: 1 },
        'not json',
      ),
    ).toThrow()
  })
  it('json_field unknown operator -- returns false', () => {
    expect(
      evaluateGate(
        { type: 'json_field', field: 'x', operator: 'unknown' as any, value: 'y', maxRetries: 1 },
        '{"x":"y"}',
      ),
    ).toBe(false)
  })

  // regex type
  it('regex match -- pass', () => {
    expect(evaluateGate({ type: 'regex', pattern: 'PASS', maxRetries: 1 }, 'Result: PASS')).toBe(
      true,
    )
  })
  it('regex miss -- fail', () => {
    expect(evaluateGate({ type: 'regex', pattern: 'PASS', maxRetries: 1 }, 'Result: FAIL')).toBe(
      false,
    )
  })
  it('regex invalid pattern -- throws', () => {
    expect(() =>
      evaluateGate({ type: 'regex', pattern: '(invalid', maxRetries: 1 }, 'test'),
    ).toThrow()
  })

  // exit_code type
  it('exit_code match -- pass', () => {
    expect(
      evaluateGate({ type: 'exit_code', value: 0, maxRetries: 1 }, 'Completed with exit code 0'),
    ).toBe(true)
  })
  it('exit_code miss -- fail', () => {
    expect(
      evaluateGate({ type: 'exit_code', value: 0, maxRetries: 1 }, 'Completed with exit code 1'),
    ).toBe(false)
  })

  // file_exists type
  it('file_exists -- pass (mocked)', () => {
    existsSyncSpy.mockReturnValueOnce(true)
    expect(evaluateGate({ type: 'file_exists', path: '/tmp/out.md', maxRetries: 1 }, '')).toBe(true)
  })
  it('file_exists -- fail (mocked)', () => {
    existsSyncSpy.mockReturnValueOnce(false)
    expect(evaluateGate({ type: 'file_exists', path: '/tmp/nope.md', maxRetries: 1 }, '')).toBe(
      false,
    )
  })
})
