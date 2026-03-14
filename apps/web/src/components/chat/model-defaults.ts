import type { ModelOption } from '../../hooks/use-models'

export const FALLBACK_MODELS: ModelOption[] = [
  {
    id: 'claude-opus-4-6',
    label: 'Claude Opus 4.6',
    description: 'Most capable for complex work',
    contextWindow: '200K',
  },
  {
    id: 'claude-sonnet-4-6',
    label: 'Claude Sonnet 4.6',
    description: 'Best for everyday tasks',
    contextWindow: '200K',
  },
  {
    id: 'claude-haiku-4-5',
    label: 'Claude Haiku 4.5',
    description: 'Fastest for quick answers',
    contextWindow: '200K',
  },
]
