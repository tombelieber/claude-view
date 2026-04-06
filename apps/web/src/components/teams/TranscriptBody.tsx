import type { TranscriptEntry } from '../../types/generated/TranscriptEntry'
import { AgentMessageCard, ModeratorCard, VerdictCard, RoundDivider } from './TranscriptCards'
import { TechnicalDetails } from './TechnicalDetails'

interface TranscriptBodyProps {
  entries: TranscriptEntry[]
  speakers: Map<string, { displayName: string; color?: string | null }>
}

const ROUND_HEADING_RE = /^#{1,3}\s+(.+)|^\*\*(.+)\*\*$/m

export function TranscriptBody({ entries, speakers }: TranscriptBodyProps) {
  const visible = entries.filter(
    (
      e,
    ): e is
      | Extract<TranscriptEntry, { kind: 'agent_message' }>
      | Extract<TranscriptEntry, { kind: 'moderator_narration' }> =>
      e.kind === 'agent_message' || e.kind === 'moderator_narration',
  )

  const elements: React.ReactNode[] = []

  for (let i = 0; i < visible.length; i++) {
    const entry = visible[i]

    if (entry.kind === 'agent_message') {
      const speaker = speakers.get(entry.teammateId)
      elements.push(
        <AgentMessageCard
          key={`agent-${entry.lineIndex}`}
          teammateId={entry.teammateId}
          displayName={speaker?.displayName ?? entry.teammateId}
          color={speaker?.color ?? entry.color}
          text={entry.text}
        />,
      )
    }

    if (entry.kind === 'moderator_narration') {
      const headingMatch = entry.text.match(ROUND_HEADING_RE)
      if (headingMatch) {
        const label = headingMatch[1] || headingMatch[2]
        elements.push(<RoundDivider key={`round-${entry.lineIndex}`} label={label} />)
      }

      if (entry.isVerdict) {
        elements.push(<VerdictCard key={`verdict-${entry.lineIndex}`} text={entry.text} />)
      } else {
        elements.push(<ModeratorCard key={`mod-${entry.lineIndex}`} text={entry.text} />)
      }
    }
  }

  return (
    <div className="space-y-4 pt-4">
      {elements}
      <TechnicalDetails entries={entries} />
    </div>
  )
}
