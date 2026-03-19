import { describe, expect, it } from 'vitest'
import { extractLastJsonBlock } from './json-extractor.js'

describe('extractLastJsonBlock', () => {
  it('extracts single code block', () => {
    const text = 'Here is the workflow:\n```json\n{"id":"test"}\n```\nDone.'
    expect(extractLastJsonBlock(text)).toBe('{"id":"test"}')
  })
  it('takes last of multiple blocks', () => {
    const text = '```json\n{"old":true}\n```\nActually:\n```json\n{"new":true}\n```'
    expect(extractLastJsonBlock(text)).toBe('{"new":true}')
  })
  it('returns null when no block', () => {
    expect(extractLastJsonBlock('No code here')).toBeNull()
  })
  it('handles whitespace-padded fences', () => {
    const text = '```json  \n{"x":1}\n  ```'
    expect(extractLastJsonBlock(text)).toBe('{"x":1}')
  })
  it('trims result', () => {
    const text = '```json\n  {"x":1}  \n```'
    expect(extractLastJsonBlock(text)).toBe('{"x":1}')
  })
  it('handles JSON containing nested code fence in a string value', () => {
    const inner = '```json\\n{\\"inner\\":1}\\n```'
    const text = '```json\n{"code":"' + inner + '"}\n```'
    const result = extractLastJsonBlock(text)
    expect(result).toContain('"code"')
  })
})
