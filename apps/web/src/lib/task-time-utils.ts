type TaskTimeLike = {
  totalTaskTimeSeconds?: number | null
  longestTaskSeconds?: number | null
  turnDurationAvgMs?: number | null
  turnDurationMaxMs?: number | null
  turnCount?: number
  durationSeconds?: number
}

const OUTLIER_TOTAL_RATIO = 2.5
const OUTLIER_LONGEST_RATIO = 6
const MIN_LONGEST_OUTLIER_SECONDS = 60 * 60

function toPositiveInt(value: number | null | undefined): number | null {
  if (value == null || !Number.isFinite(value)) return null
  const n = Math.round(value)
  return n > 0 ? n : null
}

function estimateTotalFromTurnDuration(session: TaskTimeLike): number | null {
  const avgMs = toPositiveInt(session.turnDurationAvgMs)
  const turns = toPositiveInt(session.turnCount)
  if (!avgMs || !turns) return null
  const estimated = Math.round((avgMs * turns) / 1000)
  const duration = toPositiveInt(session.durationSeconds)
  if (duration) return Math.min(estimated, duration)
  return estimated
}

function estimateLongestFromTurnDuration(session: TaskTimeLike): number | null {
  const maxMs = toPositiveInt(session.turnDurationMaxMs)
  if (!maxMs) return null
  const estimated = Math.round(maxMs / 1000)
  const duration = toPositiveInt(session.durationSeconds)
  if (duration) return Math.min(estimated, duration)
  return estimated
}

/**
 * Pick a user-facing "task time" value.
 *
 * We prefer totalTaskTimeSeconds when it looks sane. If it is a large outlier
 * versus Claude-reported turn durations, use the turn-duration estimate instead.
 */
export function getDisplayTaskTimeSeconds(session: TaskTimeLike): number | null {
  const totalTask = toPositiveInt(session.totalTaskTimeSeconds)
  const turnDurationEstimate = estimateTotalFromTurnDuration(session)
  const duration = toPositiveInt(session.durationSeconds)

  if (totalTask) {
    if (turnDurationEstimate && totalTask > turnDurationEstimate * OUTLIER_TOTAL_RATIO) {
      return turnDurationEstimate
    }
    return totalTask
  }

  if (turnDurationEstimate) return turnDurationEstimate
  return duration
}

/**
 * Pick a user-facing "longest task" value with the same outlier guard.
 */
export function getDisplayLongestTaskSeconds(session: TaskTimeLike): number | null {
  const longestTask = toPositiveInt(session.longestTaskSeconds)
  const turnDurationEstimate = estimateLongestFromTurnDuration(session)

  if (longestTask) {
    if (
      turnDurationEstimate &&
      longestTask >
        Math.max(MIN_LONGEST_OUTLIER_SECONDS, turnDurationEstimate * OUTLIER_LONGEST_RATIO)
    ) {
      return turnDurationEstimate
    }
    return longestTask
  }

  return turnDurationEstimate
}
