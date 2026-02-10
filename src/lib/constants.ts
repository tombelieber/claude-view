/**
 * Sentinel value for "no branch" in URL params and API calls.
 * Tilde (~) is invalid in git branch names, so collision is impossible.
 * Used in URL: ?branch=~ means "filter to sessions with no git branch"
 */
export const NO_BRANCH = '~' as const
export type NoBranch = typeof NO_BRANCH
