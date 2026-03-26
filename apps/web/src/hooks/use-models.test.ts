import { describe, expect, it } from 'vitest'
import { formatModelName } from '../lib/format-model'
import type { ModelOption } from './use-models'
import { buildLabel, resolveSessionModel } from './use-models'

// ---------------------------------------------------------------------------
// Real SDK response fixture (from /api/sidecar/sessions/models, 2026-03-26)
// ---------------------------------------------------------------------------
// SDK returns aliases, NOT real model IDs. "default" = whatever the current
// default model is (currently Opus 4.6). There is NO explicit "opus" entry.
// This is the root cause that the old DB-based pipeline could not handle.

const REAL_SDK_RESPONSE = {
  models: [
    {
      value: 'default',
      displayName: 'Default (recommended)',
      description: 'Opus 4.6 with 1M context · Most capable for complex work',
    },
    {
      value: 'sonnet',
      displayName: 'Sonnet',
      description: 'Sonnet 4.6 · Best for everyday tasks',
    },
    {
      value: 'haiku',
      displayName: 'Haiku',
      description: 'Haiku 4.5 · Fastest for quick answers',
    },
  ],
  updatedAt: 1774535367395,
}

// ---------------------------------------------------------------------------
// buildLabel — extracts "Claude {Family} {version}" from description
// ---------------------------------------------------------------------------

describe('buildLabel', () => {
  it('extracts "Claude Opus 4.6" from default model description', () => {
    expect(buildLabel(REAL_SDK_RESPONSE.models[0])).toBe('Claude Opus 4.6')
  })

  it('extracts "Claude Sonnet 4.6" from sonnet description', () => {
    expect(buildLabel(REAL_SDK_RESPONSE.models[1])).toBe('Claude Sonnet 4.6')
  })

  it('extracts "Claude Haiku 4.5" from haiku description', () => {
    expect(buildLabel(REAL_SDK_RESPONSE.models[2])).toBe('Claude Haiku 4.5')
  })

  it('falls back to formatModelName for unknown description format', () => {
    expect(buildLabel({ value: 'sonnet', description: 'some other format' })).toBe('Sonnet')
  })

  it('falls back to formatModelName when no description', () => {
    expect(buildLabel({ value: 'opus' })).toBe('Opus')
  })

  it('handles full model IDs via formatModelName fallback', () => {
    expect(buildLabel({ value: 'claude-opus-4-6' })).toBe('Claude Opus 4.6')
  })
})

// ---------------------------------------------------------------------------
// resolveSessionModel — matches historical real IDs to SDK aliases
// ---------------------------------------------------------------------------

describe('resolveSessionModel', () => {
  // Options as they appear after processing the real SDK response:
  // "default" becomes the opus entry, "sonnet" and "haiku" stay as-is.
  const sdkOptions: ModelOption[] = [
    { id: 'default', label: 'Claude Opus 4.6 (Default)', contextWindow: '1M' },
    { id: 'sonnet', label: 'Claude Sonnet 4.6' },
    { id: 'haiku', label: 'Claude Haiku 4.5' },
  ]

  it('exact match for alias-based model IDs', () => {
    expect(resolveSessionModel('sonnet', sdkOptions)).toBe('sonnet')
    expect(resolveSessionModel('haiku', sdkOptions)).toBe('haiku')
    expect(resolveSessionModel('default', sdkOptions)).toBe('default')
  })

  it('resolves modern real model IDs to alias via family match', () => {
    expect(resolveSessionModel('claude-sonnet-4-6', sdkOptions)).toBe('sonnet')
    expect(resolveSessionModel('claude-haiku-4-5-20251001', sdkOptions)).toBe('haiku')
  })

  it('resolves legacy real model IDs to alias via family match', () => {
    expect(resolveSessionModel('claude-3-5-sonnet-20241022', sdkOptions)).toBe('sonnet')
    expect(resolveSessionModel('claude-3-haiku-20240307', sdkOptions)).toBe('haiku')
  })

  it('returns null for opus real ID (no "opus" alias — only "default")', () => {
    // This is expected: opus is only available as "default", and we can't
    // reliably map "claude-opus-4-6" → "default" without parsing descriptions.
    // Caller falls back to user's last-used model from localStorage.
    expect(resolveSessionModel('claude-opus-4-6', sdkOptions)).toBeNull()
  })

  it('returns null for null/undefined/empty', () => {
    expect(resolveSessionModel(null, sdkOptions)).toBeNull()
    expect(resolveSessionModel(undefined, sdkOptions)).toBeNull()
    expect(resolveSessionModel('', sdkOptions)).toBeNull()
  })

  it('returns null when options list is empty', () => {
    expect(resolveSessionModel('sonnet', [])).toBeNull()
  })

  it('returns null for non-Claude models', () => {
    expect(resolveSessionModel('gpt-4o', sdkOptions)).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// Regression: the old pipeline's bug — "default" mapped to sonnet, opus lost
// ---------------------------------------------------------------------------

describe('regression: opus must appear in model list', () => {
  it('default model with opus description produces "Claude Opus 4.6" label', () => {
    const defaultModel = REAL_SDK_RESPONSE.models[0]
    expect(defaultModel.value).toBe('default')
    expect(defaultModel.description).toContain('Opus 4.6')
    expect(buildLabel(defaultModel)).toBe('Claude Opus 4.6')
  })

  it('default is NOT a duplicate of sonnet or haiku (must not be filtered out)', () => {
    const labels = REAL_SDK_RESPONSE.models.map((m) => buildLabel(m))
    // "Claude Opus 4.6", "Claude Sonnet 4.6", "Claude Haiku 4.5"
    expect(new Set(labels).size).toBe(3)
    expect(labels).toContain('Claude Opus 4.6')
  })

  it('all three model families are represented', () => {
    const labels = REAL_SDK_RESPONSE.models.map((m) => buildLabel(m))
    expect(labels).toContain('Claude Opus 4.6')
    expect(labels).toContain('Claude Sonnet 4.6')
    expect(labels).toContain('Claude Haiku 4.5')
  })
})

// ---------------------------------------------------------------------------
// formatModelName handles both aliases and real IDs
// ---------------------------------------------------------------------------

describe('formatModelName compatibility', () => {
  it('capitalizes bare aliases', () => {
    expect(formatModelName('opus')).toBe('Opus')
    expect(formatModelName('sonnet')).toBe('Sonnet')
    expect(formatModelName('haiku')).toBe('Haiku')
  })

  it('formats real model IDs with Claude prefix', () => {
    expect(formatModelName('claude-opus-4-6')).toBe('Claude Opus 4.6')
    expect(formatModelName('claude-sonnet-4-6')).toBe('Claude Sonnet 4.6')
    expect(formatModelName('claude-haiku-4-5-20251001')).toBe('Claude Haiku 4.5')
  })
})
