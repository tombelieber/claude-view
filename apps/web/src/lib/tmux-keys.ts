/**
 * Pure-function keystroke translation for CLI terminal card delegation.
 *
 * Each function translates a card action into an array of key strings
 * that are sent sequentially to the terminal via dispatchKeys().
 *
 * The 200ms inter-key delay is critical -- Ink drops keystrokes sent
 * without delay (discovered empirically in POC tests).
 */

// ANSI escape codes
const DOWN = '\x1B[B' // Down arrow
const ENTER = '\r' // Carriage return
const SPACE = ' ' // Literal space

/**
 * AskUserQuestion -- select a single option by its 0-based index.
 * Options order in JSONL matches Ink rendering order (verified by POC test-2).
 * Index 0 = first option (just Enter), index N = Down x N + Enter.
 */
export function translateSelectOption(index: number): string[] {
  const keys: string[] = []
  for (let i = 0; i < index; i++) {
    keys.push(DOWN)
  }
  keys.push(ENTER)
  return keys
}

/**
 * AskUserQuestion -- multi-select by indices.
 * Navigate to each index and press Space, then Enter to submit.
 * Indices are sorted ascending internally so callers don't need to pre-sort.
 */
export function translateMultiSelect(indices: number[]): string[] {
  const keys: string[] = []
  let currentPos = 0
  const sorted = [...indices].sort((a, b) => a - b)
  for (const idx of sorted) {
    const moves = idx - currentPos
    for (let i = 0; i < moves; i++) {
      keys.push(DOWN)
    }
    keys.push(SPACE)
    currentPos = idx
  }
  keys.push(ENTER)
  return keys
}

/**
 * Free text input (Elicitation, AskUserQuestion text input).
 * Type the text + Enter. No per-character delay needed -- Ink handles pasted text.
 */
export function translateFreeText(text: string): string[] {
  return [text, ENTER]
}

/**
 * Plan approval -- approve = Enter (default selected), reject = Down + Enter.
 */
export function translatePlanApproval(approved: boolean): string[] {
  return approved ? [ENTER] : [DOWN, ENTER]
}

/**
 * Dispatch keys to terminal with inter-key delay.
 * Ink drops keystrokes sent without delay (discovered empirically in POC tests).
 * Default 200ms delay between each key.
 */
export async function dispatchKeys(
  sendFn: (data: string) => void,
  keys: string[],
  delayMs = 200,
): Promise<void> {
  for (const key of keys) {
    sendFn(key)
    if (delayMs > 0) {
      await new Promise((resolve) => setTimeout(resolve, delayMs))
    }
  }
}
