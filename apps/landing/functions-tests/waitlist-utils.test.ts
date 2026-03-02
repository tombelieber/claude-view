import { describe, expect, it } from 'vitest'
import {
  buildShareText,
  checkHoneypot,
  generateReferralCode,
  isValidEmail,
} from '../functions/_lib/waitlist-utils'

describe('generateReferralCode', () => {
  it('generates an 8-character code', () => {
    const code = generateReferralCode()
    expect(code).toHaveLength(8)
  })

  it('uses only URL-safe characters', () => {
    const code = generateReferralCode()
    expect(code).toMatch(/^[A-Za-z0-9]+$/)
  })

  it('generates unique codes', () => {
    const codes = new Set(Array.from({ length: 100 }, () => generateReferralCode()))
    expect(codes.size).toBe(100)
  })
})

describe('isValidEmail', () => {
  it('accepts valid emails', () => {
    expect(isValidEmail('user@example.com')).toBe(true)
    expect(isValidEmail('a.b+tag@sub.domain.co')).toBe(true)
  })

  it('rejects invalid emails', () => {
    expect(isValidEmail('')).toBe(false)
    expect(isValidEmail('not-an-email')).toBe(false)
    expect(isValidEmail('@no-local.com')).toBe(false)
    expect(isValidEmail('no-domain@')).toBe(false)
    expect(isValidEmail('spaces in@email.com')).toBe(false)
  })

  it('rejects emails longer than 254 characters', () => {
    const long = 'a'.repeat(245) + '@test.com'
    expect(isValidEmail(long)).toBe(false)
  })
})

describe('checkHoneypot', () => {
  it('returns true (bot) when honeypot field is filled', () => {
    expect(checkHoneypot('some value')).toBe(true)
  })

  it('returns false (human) when honeypot is empty', () => {
    expect(checkHoneypot('')).toBe(false)
    expect(checkHoneypot(undefined)).toBe(false)
    expect(checkHoneypot(null)).toBe(false)
  })
})

describe('buildShareText', () => {
  it('includes referral link', () => {
    const text = buildShareText('Ab3xK9mQ', 'https://claudeview.ai')
    expect(text).toContain('https://claudeview.ai?ref=Ab3xK9mQ')
  })

  it('is URL-encodable for tweet intent', () => {
    const text = buildShareText('Ab3xK9mQ', 'https://claudeview.ai')
    expect(() => encodeURIComponent(text)).not.toThrow()
  })
})
