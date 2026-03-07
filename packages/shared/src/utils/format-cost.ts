/** Format a USD amount. Shows sub-cent precision for small amounts (Haiku ~$0.005). */
export function formatUsd(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.01) return `$${usd.toFixed(4).replace(/0+$/, '')}`
  return `$${usd.toFixed(2)}`
}
