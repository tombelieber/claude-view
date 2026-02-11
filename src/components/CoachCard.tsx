import { useMemo } from 'react'
import { useFluencyScore } from '../hooks/use-fluency-score'

interface CoachInsight {
  text: string
  detail: string
}

function generateInsight(score: { achievementRate: number; frictionRate: number; sessionsAnalyzed: number }): CoachInsight {
  if (score.frictionRate > 0.4) {
    return {
      text: `${Math.round(score.frictionRate * 100)}% of your sessions hit friction.`,
      detail: 'Try starting with a 1-sentence summary of what you want before diving into details.',
    }
  }
  if (score.achievementRate > 0.8) {
    return {
      text: `${Math.round(score.achievementRate * 100)}% of your sessions fully achieve the goal.`,
      detail: 'You\'re in the top tier. Keep writing detailed specs upfront.',
    }
  }
  if (score.achievementRate < 0.5) {
    return {
      text: `Only ${Math.round(score.achievementRate * 100)}% of sessions fully achieve the goal.`,
      detail: 'Include expected behavior + constraints in your first message.',
    }
  }
  return {
    text: `${score.sessionsAnalyzed} sessions analyzed.`,
    detail: 'Keep using Claude Code — your score updates weekly.',
  }
}

export function CoachCard() {
  const { data: score } = useFluencyScore()

  const insight = useMemo(() => {
    if (!score || score.score === null) return null
    return generateInsight(score)
  }, [score])

  if (!insight) {
    return (
      <div className="rounded-lg border border-border bg-card p-4">
        <p className="text-sm font-medium text-muted-foreground">Weekly Insight</p>
        <p className="text-sm mt-2">
          Run <code className="bg-muted px-1 rounded">/insights</code> in Claude Code to unlock your AI coaching.
        </p>
      </div>
    )
  }

  return (
    <div className="rounded-lg border border-border bg-card p-4">
      <p className="text-sm font-medium text-muted-foreground">Weekly Insight</p>
      <p className="text-base font-medium mt-2">{insight.text}</p>
      <p className="text-sm text-muted-foreground mt-1">{insight.detail}</p>
    </div>
  )
}
