/**
 * Pure utility functions for the waitlist CF Pages Function.
 * Underscore prefix in functions/_lib/ = not treated as a CF Pages route.
 */

const ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
const CODE_LENGTH = 8

/** Generate an 8-char URL-safe referral code using crypto.getRandomValues. */
export function generateReferralCode(): string {
  const values = crypto.getRandomValues(new Uint8Array(CODE_LENGTH))
  return Array.from(values, (v) => ALPHABET[v % ALPHABET.length]).join('')
}

/** Validate email format. No MX check — keep it fast. */
export function isValidEmail(email: string): boolean {
  if (!email || email.length >= 254) return false
  // RFC 5322 simplified: local@domain, no spaces, at least one dot in domain
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
}

/** Returns true if the honeypot field was filled (= bot). */
export function checkHoneypot(value: string | undefined | null): boolean {
  return typeof value === 'string' && value.length > 0
}

/** Build share text for X/Twitter intent. */
export function buildShareText(referralCode: string, siteUrl: string): string {
  return `I just joined the claude-view waitlist — Mission Control for AI coding agents. Join me: ${siteUrl}?ref=${referralCode}`
}
