const MODEL_CONTEXT_WINDOWS: Record<string, number> = {
  'claude-opus-4-6': 200_000,
  'claude-sonnet-4-6': 200_000,
  'claude-haiku-4-5': 200_000,
  'claude-3-opus': 200_000,
  'claude-3-sonnet': 200_000,
  'claude-3-haiku': 200_000,
  'claude-3-5-sonnet': 200_000,
  'claude-3-5-haiku': 200_000,
}

const DEFAULT_CONTEXT_WINDOW = 200_000

export function getContextWindow(model?: string | null): number {
  if (!model) return DEFAULT_CONTEXT_WINDOW
  if (MODEL_CONTEXT_WINDOWS[model]) return MODEL_CONTEXT_WINDOWS[model]
  const prefix = Object.keys(MODEL_CONTEXT_WINDOWS).find((k) => model.startsWith(k))
  return prefix ? MODEL_CONTEXT_WINDOWS[prefix] : DEFAULT_CONTEXT_WINDOW
}
