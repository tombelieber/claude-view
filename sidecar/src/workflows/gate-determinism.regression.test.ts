import { describe, expect, it, vi } from 'vitest'

vi.mock('node:fs', async (importOriginal) => ({
  ...(await importOriginal<typeof import('node:fs')>()),
  existsSync: vi.fn(() => true),
}))

const { evaluateGate } = await import('./gate-evaluator.js')

describe('gate evaluator determinism', () => {
  it('same input produces same result over 1000 iterations (json_field)', () => {
    const gate = {
      type: 'json_field' as const,
      field: 'v',
      operator: 'equals' as const,
      value: 'PASS',
      maxRetries: 1,
    }
    for (let i = 0; i < 1000; i++) {
      expect(evaluateGate(gate, '{"v":"PASS"}')).toBe(true)
    }
  })

  it('same input produces same result over 1000 iterations (regex)', () => {
    const gate = { type: 'regex' as const, pattern: 'OK$', maxRetries: 1 }
    for (let i = 0; i < 1000; i++) {
      expect(evaluateGate(gate, 'Status: OK')).toBe(true)
    }
  })

  it('same input produces same result over 1000 iterations (exit_code)', () => {
    const gate = { type: 'exit_code' as const, value: 0, maxRetries: 1 }
    for (let i = 0; i < 1000; i++) {
      expect(evaluateGate(gate, 'Completed with exit code 0')).toBe(true)
    }
  })

  it('false result is also deterministic over 1000 iterations', () => {
    const gate = {
      type: 'json_field' as const,
      field: 'v',
      operator: 'equals' as const,
      value: 'PASS',
      maxRetries: 1,
    }
    for (let i = 0; i < 1000; i++) {
      expect(evaluateGate(gate, '{"v":"FAIL"}')).toBe(false)
    }
  })
})
