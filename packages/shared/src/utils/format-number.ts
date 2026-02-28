/**
 * Smart number formatter -- always picks the unit that gives 1-3 digits before
 * the decimal. Used for line counts, generic counts, etc.
 *   999 -> "999"  |  1,200 -> "1.2K"  |  45,000 -> "45K"  |  2,300,000 -> "2.3M"  |  7.2B -> "7.20B"
 */
export function formatNumber(value: bigint | number | null): string {
  if (value === null) return '--'
  const num = typeof value === 'bigint' ? Number(value) : value
  if (num >= 1_000_000_000) {
    const b = num / 1_000_000_000
    return `${b >= 10 ? b.toFixed(1) : b.toFixed(2)}B`
  }
  if (num >= 1_000_000) return `${(num / 1_000_000).toFixed(1)}M`
  if (num >= 1_000) return `${(num / 1_000).toFixed(1)}K`
  return num.toLocaleString()
}
