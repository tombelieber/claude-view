/** Time range parameters for API calls */
export interface TimeRangeParams {
  /** Start timestamp (Unix seconds) - null for all-time */
  from: number | null
  /** End timestamp (Unix seconds) - null for all-time */
  to: number | null
}
