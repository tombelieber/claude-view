import type { TeamTranscriptBlock } from '../../../../types/blocks'
import { CollapsibleJson } from '../shared/CollapsibleJson'
import { StatusBadge } from '../shared/StatusBadge'
import { EventCard } from './EventCard'

interface Props {
  block: TeamTranscriptBlock
}

type Speaker = {
  id: string
  displayName: string
  color?: string
  stance?: string
  model?: string
}

type Entry = { kind: string; lineIndex?: number; [key: string]: unknown }

function EntryRow({ entry, speakers }: { entry: Entry; speakers: Speaker[] }) {
  const line =
    entry.lineIndex != null ? (
      <span className="text-gray-400 dark:text-gray-600 font-mono mr-1">#{entry.lineIndex}</span>
    ) : null

  switch (entry.kind) {
    case 'agent_message': {
      const speaker = speakers.find((s) => s.id === (entry.teammateId as string))
      const color = (entry.color as string | undefined) ?? speaker?.color
      const text = (entry.text as string | undefined) ?? ''
      return (
        <div className="flex items-start gap-1 text-xs">
          {line}
          <span className="font-medium flex-shrink-0" style={color ? { color } : undefined}>
            {speaker?.displayName ?? (entry.teammateId as string)}
            {speaker?.model && (
              <span className="text-gray-400 dark:text-gray-500 font-normal ml-1">
                [{speaker.model}]
              </span>
            )}
            :
          </span>
          <span className="text-gray-700 dark:text-gray-300 truncate">{text}</span>
        </div>
      )
    }
    case 'moderator_narration': {
      const isVerdict = entry.isVerdict as boolean | undefined
      return (
        <div className="flex items-start gap-1 text-xs">
          {line}
          <span
            className={
              isVerdict
                ? 'italic text-amber-700 dark:text-amber-300'
                : 'italic text-gray-500 dark:text-gray-400'
            }
          >
            {(entry.text as string | undefined) ?? ''}
          </span>
        </div>
      )
    }
    case 'moderator_relay':
      return (
        <div className="flex items-start gap-1 text-xs font-mono text-gray-500 dark:text-gray-400">
          {line}
          {'-> '}
          <span className="text-gray-700 dark:text-gray-300">{entry.to as string}</span>
          {': '}
          {entry.message as string}
        </div>
      )
    case 'task_event': {
      const status = entry.status as string | undefined
      const owner = entry.owner as string | undefined
      return (
        <div className="flex items-center gap-1.5 text-xs">
          {line}
          <span className="text-gray-700 dark:text-gray-300">{entry.subject as string}</span>
          {status && <StatusBadge label={status} />}
          {owner && <span className="text-gray-400 dark:text-gray-500">— {owner}</span>}
        </div>
      )
    }
    case 'team_lifecycle':
      return (
        <div className="flex items-start gap-1 text-xs text-gray-400 dark:text-gray-500">
          {line}
          {entry.event as string}
        </div>
      )
    case 'protocol':
      return (
        <div className="flex items-center gap-1.5 text-xs">
          {line}
          <span className="font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400">
            {(entry.msgType as string | undefined) ?? 'unknown'}
          </span>
          <CollapsibleJson data={entry.raw} label="raw" />
        </div>
      )
    default:
      return (
        <div className="flex items-start gap-1 text-xs text-gray-400 dark:text-gray-500">
          {line}
          <CollapsibleJson data={entry} label={entry.kind || 'unknown'} />
        </div>
      )
  }
}

export function DevTeamTranscriptBlock({ block }: Props) {
  const speakers = (block.speakers ?? []) as Speaker[]
  const entries = (block.entries ?? []) as Entry[]

  return (
    <EventCard
      dot="indigo"
      chip="Team"
      chipColor="bg-indigo-500/10 dark:bg-indigo-500/20 text-indigo-700 dark:text-indigo-300"
      label={`${block.teamName} — ${entries.length} entries`}
      rawData={block}
    >
      {/* Speaker list with model */}
      {speakers.length > 0 && (
        <div className="flex flex-wrap gap-2 mb-2">
          {speakers.map((s) => (
            <div key={s.id} className="flex items-center gap-1 text-xs">
              <span
                className="w-2 h-2 rounded-full flex-shrink-0"
                style={s.color ? { background: s.color } : undefined}
              />
              <span className="text-gray-700 dark:text-gray-300">{s.displayName}</span>
              {s.model && (
                <span className="text-gray-400 dark:text-gray-500 font-mono">[{s.model}]</span>
              )}
              {s.stance && (
                <span className="text-gray-400 dark:text-gray-500 italic">{s.stance}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Entry list */}
      <div className="space-y-1">
        {entries.map((entry, i) => (
          <EntryRow key={entry.lineIndex ?? i} entry={entry} speakers={speakers} />
        ))}
        {entries.length === 0 && (
          <p className="text-xs text-gray-400 dark:text-gray-500 italic">No entries</p>
        )}
      </div>
    </EventCard>
  )
}
