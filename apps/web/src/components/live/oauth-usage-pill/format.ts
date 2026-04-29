/** Human-readable reset countdown from an ISO date. Empty input → "--". */
export function formatReset(resetAt: string): string {
  if (!resetAt) return '--'
  const diffMs = new Date(resetAt).getTime() - Date.now()
  if (diffMs <= 0) return 'now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  if (hours < 24) return `${hours}h`
  return `${Math.ceil(diffMs / (1000 * 60 * 60 * 24))}d`
}

/** Longer reset label for the tooltip, showing both countdown and exact time/date. */
export function formatResetLabel(resetAt: string): string {
  if (!resetAt) return ''
  const resetDate = new Date(resetAt)
  const diffMs = resetDate.getTime() - Date.now()
  if (diffMs <= 0) return 'Resets now'
  const hours = Math.ceil(diffMs / (1000 * 60 * 60))
  const time = resetDate.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })
  if (hours < 24) return `Resets in ${hours}h · ${time}`
  const days = Math.ceil(diffMs / (1000 * 60 * 60 * 24))
  const date = resetDate.toLocaleDateString([], { month: 'short', day: 'numeric' })
  return `Resets in ${days}d · ${date}, ${time}`
}

/** Human-readable "Updated Xs ago" from a millisecond epoch timestamp. */
export function formatUpdatedAgo(epochMs: number): string {
  if (!epochMs) return ''
  const diffMs = Date.now() - epochMs
  if (diffMs < 5_000) return 'Updated just now'
  const secs = Math.floor(diffMs / 1000)
  if (secs < 60) return `Updated ${secs}s ago`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `Updated ${mins}m ago`
  return `Updated ${Math.floor(mins / 60)}h ago`
}

/** Try to extract a human-readable message from the backend error string.
 *  Backend format: `"API error 429 Too Many Requests: {\"error\":{\"message\":\"...\"}}"`. */
export function parseApiError(raw: string): { status: string; message: string } {
  const jsonStart = raw.indexOf('{')
  if (jsonStart !== -1) {
    try {
      const parsed = JSON.parse(raw.slice(jsonStart))
      const msg = parsed?.error?.message ?? parsed?.message
      if (msg) {
        const statusMatch = raw.match(/^API error (\d+ [^:]+)/)
        return { status: statusMatch?.[1] ?? 'Error', message: msg }
      }
    } catch {
      // fall through
    }
  }
  return { status: 'Error', message: raw }
}

/** Returns true if orgName is just "<email>'s Organization" — redundant info. */
export function isRedundantOrgName(orgName: string, email: string | null): boolean {
  if (!email) return false
  return (
    orgName.toLowerCase().includes(email.split('@')[0].toLowerCase()) &&
    orgName.toLowerCase().endsWith("'s organization")
  )
}
