import { describe, expect, it } from 'vitest'
import { formatModelName } from '../lib/format-model'
import type { ModelWithStats } from '../types/generated'
import type { ModelOption } from './use-models'
import { resolveSessionModel } from './use-models'

// === resolveSessionModel ===

describe('resolveSessionModel', () => {
  const sdkOptions: ModelOption[] = [
    { id: 'claude-opus-4-6', label: 'Claude Opus 4.6', contextWindow: '1M' },
    { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6', contextWindow: '200K' },
    { id: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5', contextWindow: '200K' },
  ]

  it('returns exact match when session model is SDK-supported', () => {
    expect(resolveSessionModel('claude-opus-4-6', sdkOptions)).toBe('claude-opus-4-6')
    expect(resolveSessionModel('claude-sonnet-4-6', sdkOptions)).toBe('claude-sonnet-4-6')
    expect(resolveSessionModel('claude-haiku-4-5-20251001', sdkOptions)).toBe(
      'claude-haiku-4-5-20251001',
    )
  })

  it('returns null for unsupported legacy model (caller uses user default)', () => {
    expect(resolveSessionModel('claude-sonnet-4-20250514', sdkOptions)).toBeNull()
    expect(resolveSessionModel('claude-3-5-sonnet-20241022', sdkOptions)).toBeNull()
    expect(resolveSessionModel('claude-3-opus-20240229', sdkOptions)).toBeNull()
  })

  it('returns null for null/undefined primaryModel', () => {
    expect(resolveSessionModel(null, sdkOptions)).toBeNull()
    expect(resolveSessionModel(undefined, sdkOptions)).toBeNull()
  })

  it('returns null for empty string primaryModel', () => {
    expect(resolveSessionModel('', sdkOptions)).toBeNull()
  })

  it('returns null when options list is empty (SDK not loaded yet)', () => {
    expect(resolveSessionModel('claude-opus-4-6', [])).toBeNull()
  })
})

// === Regression: display_name=null falls back to formatModelName → proper names ===

describe('formatModelName fallback produces proper display names', () => {
  it('generates "Claude Opus 4.6" from real model ID (not SDK alias)', () => {
    expect(formatModelName('claude-opus-4-6')).toBe('Claude Opus 4.6')
  })

  it('generates "Claude Sonnet 4.6" from real model ID', () => {
    expect(formatModelName('claude-sonnet-4-6')).toBe('Claude Sonnet 4.6')
  })

  it('generates "Claude Haiku 4.5" from real model ID', () => {
    expect(formatModelName('claude-haiku-4-5-20251001')).toBe('Claude Haiku 4.5')
  })

  it('SDK alias "Default (recommended)" must NEVER appear as a model label', () => {
    // If display_name is null, the hook uses formatModelName(id).
    // This test guards against anyone re-introducing SDK aliases as display names.
    const badAliases = ['Default (recommended)', 'Sonnet', 'Haiku', 'default', 'sonnet', 'haiku']
    const realIds = ['claude-opus-4-6', 'claude-sonnet-4-6', 'claude-haiku-4-5-20251001']
    for (const id of realIds) {
      const name = formatModelName(id)
      for (const alias of badAliases) {
        expect(name).not.toBe(alias)
      }
      expect(name).toMatch(/^Claude /) // must start with "Claude "
    }
  })
})

// === SDK filtering logic (pure function extraction for testability) ===

/** Extracted from useModelOptions — pure function for testing without React hooks. */
function filterModelOptions(data: ModelWithStats[], sdkOnly: boolean): ModelOption[] {
  if (data.length === 0) return []

  const hasAnySdkModel = sdkOnly && data.some((m) => m.sdkSupported)
  const effectiveSdkOnly = sdkOnly && hasAnySdkModel

  const seen = new Set<string>()
  const options: ModelOption[] = []

  for (const m of data) {
    if (!m.id.startsWith('claude-')) continue
    if (effectiveSdkOnly && !m.sdkSupported) continue
    const family = m.family ?? 'unknown'
    if (seen.has(family)) continue
    seen.add(family)
    options.push({
      id: m.id,
      label: m.displayName ?? m.id,
      description: m.description ?? undefined,
      contextWindow: m.maxInputTokens ? `${m.maxInputTokens}` : undefined,
    })
  }

  return options
}

function makeModel(overrides: Partial<ModelWithStats>): ModelWithStats {
  return {
    id: 'claude-test',
    provider: 'anthropic',
    family: 'test',
    displayName: null,
    description: null,
    maxInputTokens: null,
    maxOutputTokens: null,
    firstSeen: null,
    lastSeen: null,
    totalTurns: 0,
    totalSessions: 0,
    sdkSupported: false,
    ...overrides,
  }
}

describe('SDK model filtering (sdkOnly)', () => {
  const allModels: ModelWithStats[] = [
    makeModel({ id: 'claude-opus-4-6', family: 'opus', sdkSupported: true }),
    makeModel({ id: 'claude-sonnet-4-6', family: 'sonnet', sdkSupported: true }),
    makeModel({ id: 'claude-haiku-4-5-20251001', family: 'haiku', sdkSupported: true }),
    makeModel({ id: 'claude-3-5-sonnet-20241022', family: 'sonnet', sdkSupported: false }),
    makeModel({ id: 'claude-3-opus-20240229', family: 'opus', sdkSupported: false }),
    makeModel({ id: 'claude-3-haiku-20240307', family: 'haiku', sdkSupported: false }),
  ]

  it('filters to SDK-supported models only when sdkOnly=true', () => {
    const result = filterModelOptions(allModels, true)
    expect(result.map((o) => o.id)).toEqual([
      'claude-opus-4-6',
      'claude-sonnet-4-6',
      'claude-haiku-4-5-20251001',
    ])
  })

  it('shows all Claude models when sdkOnly=false', () => {
    const result = filterModelOptions(allModels, false)
    // Deduplicates by family — picks first per family (already sorted)
    expect(result.map((o) => o.id)).toEqual([
      'claude-opus-4-6',
      'claude-sonnet-4-6',
      'claude-haiku-4-5-20251001',
    ])
  })

  it('deduplicates by family — first model per family wins', () => {
    const models: ModelWithStats[] = [
      makeModel({
        id: 'claude-sonnet-4-6',
        family: 'sonnet',
        sdkSupported: false,
        totalTurns: 100,
      }),
      makeModel({
        id: 'claude-3-5-sonnet-20241022',
        family: 'sonnet',
        sdkSupported: false,
        totalTurns: 50,
      }),
    ]
    const result = filterModelOptions(models, false)
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('claude-sonnet-4-6')
  })
})

describe('cold-start fallback (no sdk_supported models)', () => {
  const coldStartModels: ModelWithStats[] = [
    makeModel({ id: 'claude-opus-4-6', family: 'opus', sdkSupported: false }),
    makeModel({ id: 'claude-sonnet-4-6', family: 'sonnet', sdkSupported: false }),
    makeModel({ id: 'claude-haiku-4-5-20251001', family: 'haiku', sdkSupported: false }),
    makeModel({ id: 'claude-3-5-sonnet-20241022', family: 'sonnet', sdkSupported: false }),
  ]

  it('shows all Claude models when NO model has sdkSupported=true (cold start)', () => {
    const result = filterModelOptions(coldStartModels, true)
    // Should NOT be empty — falls back to showing all
    expect(result.length).toBeGreaterThan(0)
    // All families present
    expect(result.map((o) => o.id)).toContain('claude-opus-4-6')
  })

  it('transitions to SDK-only once any model gets sdkSupported=true', () => {
    const withOneSdk = [
      ...coldStartModels,
      makeModel({ id: 'claude-opus-4-6-v2', family: 'opus-v2', sdkSupported: true }),
    ]
    const result = filterModelOptions(withOneSdk, true)
    // Now only SDK-supported models should appear
    expect(
      result.every((o) => {
        const match = withOneSdk.find((m) => m.id === o.id)
        return match?.sdkSupported
      }),
    ).toBe(true)
  })

  it('excludes non-claude models', () => {
    const models = [
      makeModel({ id: 'gpt-4o', family: 'gpt-4', sdkSupported: false }),
      makeModel({ id: 'claude-opus-4-6', family: 'opus', sdkSupported: false }),
    ]
    const result = filterModelOptions(models, false)
    expect(result.map((o) => o.id)).toEqual(['claude-opus-4-6'])
  })
})
