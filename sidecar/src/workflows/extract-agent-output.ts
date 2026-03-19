/**
 * Normalizes an agent tool response into a plain text string.
 * Handles: raw strings, content-array objects (Agent SDK format),
 * plain text-field objects, and arbitrary objects (JSON-stringified).
 */
export function extractAgentOutput(toolResponse: unknown): string {
  if (typeof toolResponse === 'string') return toolResponse
  if (toolResponse == null) return ''

  if (typeof toolResponse === 'object') {
    const resp = toolResponse as Record<string, unknown>

    if (Array.isArray(resp.content)) {
      const texts = (resp.content as Array<{ type?: string; text?: string }>)
        .filter((block) => block.type === 'text' && typeof block.text === 'string')
        .map((block) => block.text!)

      if (texts.length > 0) return texts.join('\n')
    }

    if (typeof resp.text === 'string') return resp.text

    return JSON.stringify(toolResponse)
  }

  return ''
}
