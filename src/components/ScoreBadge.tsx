import { useFluencyScore } from '../hooks/use-fluency-score'

export function ScoreBadge() {
  const { data: score, isLoading } = useFluencyScore()

  if (isLoading || !score || score.score === null) {
    return (
      <span
        className="text-xs text-gray-400 dark:text-gray-500"
        title="Run /insights in Claude Code to unlock"
      >
        Score: --
      </span>
    )
  }

  const color =
    score.score >= 70
      ? 'text-green-500'
      : score.score >= 40
        ? 'text-amber-500'
        : 'text-red-500'

  return (
    <span
      className={`text-sm font-semibold ${color}`}
      title={`Based on ${score.sessionsAnalyzed} sessions`}
    >
      Score: {score.score}
    </span>
  )
}
