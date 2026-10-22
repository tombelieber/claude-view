interface RequestHeaders {
  host?: string | string[]
  origin?: string | string[]
}

function isLoopbackAuthority(value: string): boolean {
  const match = /^(?:localhost|127\.0\.0\.1|\[::1\])(?::(\d{1,5}))?$/i.exec(value)
  return (
    match !== null && match[0] === value && (match[1] === undefined || Number(match[1]) <= 65535)
  )
}

/**
 * The sidecar is a local control API. Check Host as well as browser Origin:
 * a loopback bind alone does not prevent DNS rebinding or cross-site WebSockets.
 * Rust proxy and relay connections have no Origin and remain supported.
 */
export function isLocalRequest({ host, origin }: RequestHeaders): boolean {
  if (typeof host !== 'string' || !isLoopbackAuthority(host)) return false
  if (origin === undefined) return true
  if (typeof origin !== 'string') return false

  const match = /^https?:\/\/(.+)$/i.exec(origin)
  return match !== null && match[0] === origin && isLoopbackAuthority(match[1])
}
