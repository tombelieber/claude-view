import { useSearchParams } from 'react-router-dom'
import { useMemo } from 'react'
import { NO_BRANCH } from '../lib/constants'

/**
 * Shared hook for decoding the branch URL parameter.
 * Maps the sentinel value ~ to null for API calls, and provides
 * boolean flags for easy conditional logic.
 */
export function useBranchFilter() {
  const [searchParams] = useSearchParams()

  return useMemo(() => {
    const raw = searchParams.get('branch')
    return {
      /** Raw URL param value (null, "~", or branch name) */
      urlBranch: raw,
      /** Value to send to API: null when no filter or filtering by no-branch, string for named branch */
      apiBranch: raw === NO_BRANCH ? undefined : (raw || undefined),
      /** Whether we're filtering to sessions with no branch */
      isNoBranch: raw === NO_BRANCH,
      /** Whether any branch filter is active */
      isFiltered: raw !== null && raw !== '',
      /** The sentinel constant for use in comparisons */
      NO_BRANCH,
    }
  }, [searchParams])
}
