/**
 * Replaces {{varName}} placeholders in a template string with values from inputValues.
 * Throws if a declared input variable is referenced but not provided.
 * Escaped braces (\\{\\{ and \\}\\}) are preserved as literal {{ and }}.
 */
export function interpolatePrompt(
  template: string,
  _inputDefs: Array<{ name: string; type: string }>,
  inputValues: Record<string, string>,
): string {
  const OPEN = '\u{FFFE}OPEN\u{FFFE}'
  const CLOSE = '\u{FFFE}CLOSE\u{FFFE}'

  // Replace escaped braces with sentinel tokens first
  const escaped = template.replaceAll('\\{\\{', OPEN).replaceAll('\\}\\}', CLOSE)

  const result = escaped.replace(/\{\{(\w+(?:\.\w+)*)\}\}/g, (_match, varName: string) => {
    const value = inputValues[varName]
    if (value === undefined) throw new Error(`Missing input: ${varName}`)
    return value
  })

  return result.replaceAll(OPEN, '{{').replaceAll(CLOSE, '}}')
}
