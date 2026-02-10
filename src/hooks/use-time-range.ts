import { useState, useEffect, useMemo, useCallback } from 'react'
import { useSearchParams } from 'react-router-dom'

/** Predefined time range options */
export type TimeRangePreset = 'today' | '7d' | '30d' | '90d' | 'all' | 'custom'

/** Custom date range with from/to dates */
export interface CustomDateRange {
  from: Date
  to: Date
}

/** Time range state including computed timestamps */
export interface TimeRangeState {
  /** Current preset selection */
  preset: TimeRangePreset
  /** Custom date range (only used when preset is 'custom') */
  customRange: CustomDateRange | null
  /** Computed 'from' timestamp in seconds (null for 'all') */
  fromTimestamp: number | null
  /** Computed 'to' timestamp in seconds (null for 'all') */
  toTimestamp: number | null
}

/** Hook return type */
export interface UseTimeRangeReturn {
  /** Current time range state */
  state: TimeRangeState
  /** Set preset (7d, 30d, 90d, all) */
  setPreset: (preset: TimeRangePreset) => void
  /** Set custom date range */
  setCustomRange: (range: CustomDateRange | null) => void
  /** Human-readable label for the current range */
  label: string
  /** Period label for comparison (e.g., "vs prev 7d") */
  comparisonLabel: string | null
}

const STORAGE_KEY = 'dashboard-time-range'

/** Map old Contributions-page URL params to new unified presets.
 *  Keeps bookmarked /contributions?range=week URLs working after migration. */
const LEGACY_RANGE_MAP: Record<string, TimeRangePreset> = {
  week: '7d',
  month: '30d',
  '90days': '90d',
}

/** Calculate timestamps from preset */
function getTimestampsFromPreset(preset: TimeRangePreset): { from: number | null; to: number | null } {
  if (preset === 'all') {
    return { from: null, to: null }
  }

  const now = Math.floor(Date.now() / 1000)

  if (preset === 'today') {
    // Since midnight today (matches backend's TimeRange::Today semantics)
    const midnight = new Date()
    midnight.setHours(0, 0, 0, 0)
    return { from: Math.floor(midnight.getTime() / 1000), to: now }
  }

  const days = preset === '7d' ? 7 : preset === '30d' ? 30 : 90
  const from = now - days * 86400
  return { from, to: now }
}

/** Calculate timestamps from custom date range */
function getTimestampsFromCustomRange(range: CustomDateRange | null): { from: number | null; to: number | null } {
  if (!range) {
    return { from: null, to: null }
  }
  // Set from to start of day, to to end of day
  const fromDate = new Date(range.from)
  fromDate.setHours(0, 0, 0, 0)
  const toDate = new Date(range.to)
  toDate.setHours(23, 59, 59, 999)

  return {
    from: Math.floor(fromDate.getTime() / 1000),
    to: Math.floor(toDate.getTime() / 1000),
  }
}

/** Format date for display */
function formatDate(date: Date): string {
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

/** Format timestamp for display */
function formatTimestamp(ts: number): string {
  return formatDate(new Date(ts * 1000))
}

/**
 * useTimeRange - Manages time range state with URL sync and localStorage persistence.
 *
 * State is synchronized:
 * 1. URL params take precedence: ?range=30d or ?from=...&to=...
 * 2. Falls back to localStorage
 * 3. Defaults to '30d' if neither exists
 *
 * Updates both URL and localStorage on change.
 */
export function useTimeRange(): UseTimeRangeReturn {
  const [searchParams, setSearchParams] = useSearchParams()

  // Initialize state from URL or localStorage
  const [preset, setPresetState] = useState<TimeRangePreset>(() => {
    // Check URL params first
    const rangeParam = searchParams.get('range')
    if (rangeParam) {
      const migrated = LEGACY_RANGE_MAP[rangeParam]
      if (migrated) return migrated
      if (['today', '7d', '30d', '90d', 'all', 'custom'].includes(rangeParam)) {
        return rangeParam as TimeRangePreset
      }
    }
    // Check if custom timestamps are in URL
    if (searchParams.get('from') && searchParams.get('to')) {
      return 'custom'
    }
    // Fall back to localStorage
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const parsed = JSON.parse(stored)
        if (parsed.preset && ['today', '7d', '30d', '90d', 'all', 'custom'].includes(parsed.preset)) {
          return parsed.preset as TimeRangePreset
        }
      }
    } catch (e) {
      console.warn('Failed to read time range from localStorage:', e)
    }
    // Default to 30d
    return '30d'
  })

  const [customRange, setCustomRangeState] = useState<CustomDateRange | null>(() => {
    // Check URL params for custom range
    const fromParam = searchParams.get('from')
    const toParam = searchParams.get('to')
    if (fromParam && toParam) {
      const from = parseInt(fromParam, 10)
      const to = parseInt(toParam, 10)
      if (!isNaN(from) && !isNaN(to)) {
        return {
          from: new Date(from * 1000),
          to: new Date(to * 1000),
        }
      }
    }
    // Fall back to localStorage
    try {
      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const parsed = JSON.parse(stored)
        if (parsed.customRange?.from && parsed.customRange?.to) {
          return {
            from: new Date(parsed.customRange.from),
            to: new Date(parsed.customRange.to),
          }
        }
      }
    } catch (e) {
      console.warn('Failed to read time range from localStorage:', e)
    }
    return null
  })

  // Compute timestamps based on current state
  const timestamps = useMemo(() => {
    if (preset === 'custom') {
      return getTimestampsFromCustomRange(customRange)
    }
    return getTimestampsFromPreset(preset)
  }, [preset, customRange])

  // Stable primitive key for searchParams to avoid re-renders from object identity
  const urlKey = searchParams.toString()

  // Sync to URL and localStorage when state changes
  useEffect(() => {
    // Update localStorage
    const toStore = {
      preset,
      customRange: customRange ? {
        from: customRange.from.toISOString(),
        to: customRange.to.toISOString(),
      } : null,
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(toStore))

    // Update URL params
    const newParams = new URLSearchParams(searchParams)

    // Clear existing time params
    newParams.delete('range')
    newParams.delete('from')
    newParams.delete('to')

    if (preset === 'custom' && customRange) {
      const { from, to } = getTimestampsFromCustomRange(customRange)
      if (from !== null && to !== null) {
        newParams.set('from', from.toString())
        newParams.set('to', to.toString())
      }
    } else if (preset !== '30d') {
      // Only set URL param if not default
      newParams.set('range', preset)
    }

    // Only update if params actually changed
    if (newParams.toString() !== urlKey) {
      setSearchParams(newParams, { replace: true })
    }
  }, [preset, customRange, urlKey, searchParams, setSearchParams])

  // Handlers
  const setPreset = useCallback((newPreset: TimeRangePreset) => {
    setPresetState(newPreset)
    // Clear custom range when switching to a preset
    if (newPreset !== 'custom') {
      setCustomRangeState(null)
    }
  }, [])

  const setCustomRange = useCallback((range: CustomDateRange | null) => {
    setCustomRangeState(range)
    if (range) {
      setPresetState('custom')
    }
  }, [])

  // Compute labels
  const label = useMemo(() => {
    if (preset === 'today') {
      return 'Today'
    }
    if (preset === 'all') {
      return 'All time'
    }
    if (preset === 'custom' && customRange) {
      return `${formatDate(customRange.from)} - ${formatDate(customRange.to)}`
    }
    if (timestamps.from !== null && timestamps.to !== null) {
      return `${formatTimestamp(timestamps.from)} - ${formatTimestamp(timestamps.to)}`
    }
    return preset
  }, [preset, customRange, timestamps])

  const comparisonLabel = useMemo(() => {
    if (preset === 'today') {
      return 'vs yesterday'
    }
    if (preset === 'all') {
      return null // No comparison for all-time
    }
    if (preset === 'custom' && customRange) {
      const days = Math.ceil((customRange.to.getTime() - customRange.from.getTime()) / (1000 * 86400))
      return `vs prev ${days}d`
    }
    return `vs prev ${preset}`
  }, [preset, customRange])

  // Build state object
  const state: TimeRangeState = {
    preset,
    customRange,
    fromTimestamp: timestamps.from,
    toTimestamp: timestamps.to,
  }

  return {
    state,
    setPreset,
    setCustomRange,
    label,
    comparisonLabel,
  }
}
