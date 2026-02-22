import { describe, it, expect } from 'vitest'
import {
  tryParseJson,
  isJsonContent,
  isDiffContent,
  isCodeLikeContent,
  stripLineNumbers,
  detectCodeLanguage,
  shortenToolName,
  toolChipColor,
} from './content-detection'

describe('tryParseJson', () => {
  it('parses valid JSON object', () => {
    const result = tryParseJson('{"key": "value", "num": 42}')
    expect(result).toEqual({ key: 'value', num: 42 })
  })

  it('parses valid JSON array', () => {
    const result = tryParseJson('[1, 2, 3]')
    expect(result).toEqual([1, 2, 3])
  })

  it('returns null for plain text', () => {
    expect(tryParseJson('hello world')).toBeNull()
  })

  it('returns null for empty string', () => {
    expect(tryParseJson('')).toBeNull()
  })

  it('returns null for whitespace-only string', () => {
    expect(tryParseJson('   ')).toBeNull()
  })

  it('returns null for invalid JSON that starts with {', () => {
    expect(tryParseJson('{not json}')).toBeNull()
  })

  it('handles leading/trailing whitespace in valid JSON', () => {
    const result = tryParseJson('  {"a": 1}  ')
    expect(result).toEqual({ a: 1 })
  })
})

describe('isJsonContent', () => {
  it('returns true for valid JSON object', () => {
    expect(isJsonContent('{"key": "value"}')).toBe(true)
  })

  it('returns false for plain text', () => {
    expect(isJsonContent('just some text')).toBe(false)
  })
})

describe('isDiffContent', () => {
  it('detects diff --git header', () => {
    expect(isDiffContent('diff --git a/file.ts b/file.ts\n--- a/file.ts\n+++ b/file.ts')).toBe(true)
  })

  it('detects --- header (patch format)', () => {
    expect(isDiffContent('--- a/file.ts\n+++ b/file.ts\n@@ -1,3 +1,3 @@')).toBe(true)
  })

  it('detects Index: header', () => {
    expect(isDiffContent('Index: file.ts\n===\n--- file.ts\n+++ file.ts')).toBe(true)
  })

  it('detects high density of +/- lines', () => {
    const content = [
      '@@ -1,5 +1,5 @@',
      '-old line 1',
      '-old line 2',
      '+new line 1',
      '+new line 2',
      ' context line',
    ].join('\n')
    expect(isDiffContent(content)).toBe(true)
  })

  it('rejects normal text', () => {
    expect(isDiffContent('This is just normal text.\nNothing special here.\nAnother line.')).toBe(false)
  })

  it('rejects text with too few lines', () => {
    expect(isDiffContent('short')).toBe(false)
  })
})

describe('isCodeLikeContent', () => {
  it('detects cat -n style line numbers with arrow', () => {
    const content = [
      '  1\u2192import React from "react"',
      '  2\u2192import { useState } from "react"',
      '  3\u2192',
      '  4\u2192export function App() {',
    ].join('\n')
    expect(isCodeLikeContent(content)).toBe(true)
  })

  it('detects tab-separated line numbers', () => {
    const content = [
      '1\timport foo',
      '2\timport bar',
      '3\tconst x = 1',
    ].join('\n')
    expect(isCodeLikeContent(content)).toBe(true)
  })

  it('rejects normal text without line numbers', () => {
    expect(isCodeLikeContent('This is just text.\nAnother line.')).toBe(false)
  })

  it('rejects single line content', () => {
    expect(isCodeLikeContent('  1\u2192single line')).toBe(false)
  })
})

describe('stripLineNumbers', () => {
  it('strips arrow-style line number prefixes', () => {
    const input = '  1\u2192import React\n  2\u2192const x = 1'
    const result = stripLineNumbers(input)
    expect(result).toBe('import React\nconst x = 1')
  })

  it('strips tab-style line number prefixes', () => {
    const input = '1\tline one\n2\tline two'
    const result = stripLineNumbers(input)
    expect(result).toBe('line one\nline two')
  })

  it('leaves content without line numbers unchanged', () => {
    const input = 'no line numbers here'
    expect(stripLineNumbers(input)).toBe(input)
  })
})

describe('detectCodeLanguage', () => {
  it('detects diff content', () => {
    expect(detectCodeLanguage('diff --git a/f b/f\n--- a/f\n+++ b/f')).toBe('diff')
  })

  it('detects Rust code', () => {
    expect(detectCodeLanguage('use std::io;\nfn main() {\n  println!("hello");\n}')).toBe('rust')
  })

  it('detects TypeScript code', () => {
    expect(detectCodeLanguage('import { foo } from "bar"\nexport const x: string = "hello"')).toBe('typescript')
  })

  it('returns text for unrecognized content', () => {
    expect(detectCodeLanguage('just some random text\nnothing special')).toBe('text')
  })
})

describe('shortenToolName', () => {
  it('extracts tool name and server from MCP tool name', () => {
    const result = shortenToolName('mcp__chrome-devtools__take_snapshot')
    expect(result).toEqual({ short: 'take_snapshot', server: 'chrome-devtools' })
  })

  it('handles MCP tools with underscores in server name', () => {
    const result = shortenToolName('mcp__my_server__do_thing')
    expect(result).toEqual({ short: 'do_thing', server: 'my_server' })
  })

  it('returns as-is for non-MCP tool name', () => {
    const result = shortenToolName('Read')
    expect(result).toEqual({ short: 'Read' })
  })

  it('returns as-is for Bash', () => {
    const result = shortenToolName('Bash')
    expect(result).toEqual({ short: 'Bash' })
  })

  it('returns as-is for empty string', () => {
    const result = shortenToolName('')
    expect(result).toEqual({ short: '' })
  })
})

describe('toolChipColor', () => {
  it('returns blue for MCP tools', () => {
    const color = toolChipColor('mcp__chrome-devtools__take_snapshot')
    expect(color).toContain('blue')
  })

  it('returns purple for Skill', () => {
    const color = toolChipColor('Skill')
    expect(color).toContain('purple')
  })

  it('returns indigo for Task', () => {
    const color = toolChipColor('Task')
    expect(color).toContain('indigo')
  })

  it('returns emerald for Read/Glob/Grep', () => {
    expect(toolChipColor('Read')).toContain('emerald')
    expect(toolChipColor('Glob')).toContain('emerald')
    expect(toolChipColor('Grep')).toContain('emerald')
  })

  it('returns amber for Write/Edit', () => {
    expect(toolChipColor('Write')).toContain('amber')
    expect(toolChipColor('Edit')).toContain('amber')
  })

  it('returns gray for Bash', () => {
    expect(toolChipColor('Bash')).toContain('gray')
  })

  it('returns orange for unknown tools', () => {
    const color = toolChipColor('SomeUnknownTool')
    expect(color).toContain('orange')
  })
})
