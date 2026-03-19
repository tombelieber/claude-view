import { describe, expect, it } from 'vitest'
import { extractAgentOutput } from './extract-agent-output.js'

describe('extractAgentOutput', () => {
  it('returns string input directly', () => {
    expect(extractAgentOutput('hello')).toBe('hello')
  })
  it('extracts text from AgentOutput content array', () => {
    const output = { content: [{ type: 'text', text: 'Result: PASS' }], status: 'completed' }
    expect(extractAgentOutput(output)).toBe('Result: PASS')
  })
  it('joins multiple text blocks', () => {
    const output = {
      content: [
        { type: 'text', text: 'A' },
        { type: 'text', text: 'B' },
      ],
    }
    expect(extractAgentOutput(output)).toBe('A\nB')
  })
  it('filters non-text blocks', () => {
    const output = {
      content: [
        { type: 'image', data: '...' },
        { type: 'text', text: 'only this' },
      ],
    }
    expect(extractAgentOutput(output)).toBe('only this')
  })
  it('falls back to plain text field', () => {
    expect(extractAgentOutput({ text: 'fallback' })).toBe('fallback')
  })
  it('stringifies unknown objects', () => {
    const result = extractAgentOutput({ unknown: 'field' })
    expect(result).toBe('{"unknown":"field"}')
  })
  it('returns empty string for null', () => {
    expect(extractAgentOutput(null)).toBe('')
  })
  it('returns empty string for undefined', () => {
    expect(extractAgentOutput(undefined)).toBe('')
  })
  it('handles empty content array (falls through to stringify)', () => {
    const result = extractAgentOutput({ content: [] })
    expect(result).toBe('{"content":[]}')
  })
})
