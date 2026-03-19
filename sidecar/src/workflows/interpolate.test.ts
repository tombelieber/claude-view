import { describe, expect, it } from 'vitest'
import { interpolatePrompt } from './interpolate.js'

describe('interpolatePrompt', () => {
  it('replaces simple {{var}}', () => {
    expect(
      interpolatePrompt('Hello {{name}}', [{ name: 'name', type: 'string' }], { name: 'World' }),
    ).toBe('Hello World')
  })

  it('replaces multiple vars', () => {
    expect(
      interpolatePrompt(
        '{{a}} and {{b}}',
        [
          { name: 'a', type: 'string' },
          { name: 'b', type: 'string' },
        ],
        { a: 'X', b: 'Y' },
      ),
    ).toBe('X and Y')
  })

  it('throws on missing var', () => {
    expect(() =>
      interpolatePrompt('{{missing}}', [{ name: 'missing', type: 'string' }], {}),
    ).toThrow('Missing input: missing')
  })

  it('handles empty string var', () => {
    expect(interpolatePrompt('{{x}}', [{ name: 'x', type: 'string' }], { x: '' })).toBe('')
  })

  it('preserves escaped braces \\{\\{', () => {
    expect(interpolatePrompt('\\{\\{literal\\}\\}', [], {})).toBe('{{literal}}')
  })
})
