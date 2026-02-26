interface QualityBadgeProps {
  outcome?: string | null
  satisfaction?: string | null
}

export function QualityBadge({ outcome, satisfaction }: QualityBadgeProps) {
  if (!outcome && !satisfaction) {
    return <span className="inline-block w-2 h-2 rounded-full bg-zinc-700" title="No quality data" />
  }

  const isGood = (outcome === 'fully_achieved' || outcome === 'mostly_achieved') &&
    (satisfaction === 'satisfied' || satisfaction === 'likely_satisfied' || satisfaction === 'happy')
  const isBad = outcome === 'not_achieved' || satisfaction === 'frustrated' || satisfaction === 'dissatisfied'

  const color = isGood ? 'bg-green-500' : isBad ? 'bg-red-500' : 'bg-amber-500'
  const label = isGood ? 'Good session' : isBad ? 'Friction-heavy session' : 'Mixed results'

  return <span className={`inline-block w-2 h-2 rounded-full ${color}`} title={label} />
}
