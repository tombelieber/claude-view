import type { TeamTranscriptBlock } from '../../../../types/blocks'
import { Users } from 'lucide-react'
import { CollapsibleJson } from '../shared/CollapsibleJson'
import { StatusBadge } from '../shared/StatusBadge'

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

type Entry = { kind: string; [key: string]: unknown }

function truncate(text: string, max: number): string {
  return text.length > max ? `${text.slice(0, max)}…` : text
}

function EntryRow({ entry, speakers }: { entry: Entry; speakers: Speaker[] }) {
  switch (entry.kind) {
    case 'agent_message': {
      const speaker = speakers.find((s) => s.id === (entry.teammateId as string))
      const color = (entry.color as string | undefined) ?? speaker?.color
      const text = (entry.text as string | undefined) ?? ''
      const summary = entry.summary as string | undefined
      return (
        <div className="flex items-start gap-1.5 text-xs">
          <span className="font-medium flex-shrink-0" style={color ? { color } : undefined}>
            {speaker?.displayName ?? (entry.teammateId as string)}:
          </span>
          <span className="text-gray-700 dark:text-gray-300 truncate">{truncate(text, 80)}</span>
          {summary && (
            <span className="text-gray-400 dark:text-gray-500 italic flex-shrink-0 ml-1">
              {summary}
            </span>
          )}
        </div>
      )
    }

    case 'moderator_narration': {
      const isVerdict = entry.isVerdict as boolean | undefined
      const text = (entry.text as string | undefined) ?? ''
      return (
        <div
          className={
            isVerdict
              ? 'text-xs italic px-2 py-0.5 rounded bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-300'
              : 'text-xs italic text-gray-500 dark:text-gray-400'
          }
        >
          {text}
        </div>
      )
    }

    case 'moderator_relay': {
      const to = (entry.to as string | undefined) ?? ''
      const message = (entry.message as string | undefined) ?? ''
      return (
        <div className="text-xs text-gray-500 dark:text-gray-400 font-mono">
          {'-> '}
          <span className="text-gray-700 dark:text-gray-300">{to}</span>
          {': '}
          {message}
        </div>
      )
    }

    case 'task_event': {
      const subject = (entry.subject as string | undefined) ?? ''
      const status = entry.status as string | undefined
      const owner = entry.owner as string | undefined
      return (
        <div className="flex items-center gap-1.5 text-xs">
          <span className="text-gray-700 dark:text-gray-300">{subject}</span>
          {status && <StatusBadge label={status} />}
          {owner && <span className="text-gray-400 dark:text-gray-500">— {owner}</span>}
        </div>
      )
    }

    case 'team_lifecycle': {
      const event = (entry.event as string | undefined) ?? ''
      return <div className="text-xs text-gray-400 dark:text-gray-500">{event}</div>
    }

    case 'protocol': {
      const msgType = (entry.msgType as string | undefined) ?? 'unknown'
      return (
        <div className="flex items-center gap-1.5 text-xs">
          <span className="font-mono px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400">
            {msgType}
          </span>
          <CollapsibleJson data={entry.raw} label="raw" />
        </div>
      )
    }

    default:
      return (
        <div className="text-xs text-gray-400 dark:text-gray-500">
          <CollapsibleJson data={entry} label={entry.kind || 'unknown'} />
        </div>
      )
  }
}

export function ChatTeamTranscriptBlock({ block }: Props) {
  const speakers = (block.speakers ?? []) as Speaker[]
  const entries = (block.entries ?? []) as Entry[]

  return (
    <div className="rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      {/* Header */}
      <div className="px-3 py-2 bg-gray-50 dark:bg-gray-800/50 border-b border-gray-200 dark:border-gray-700">
        <div className="flex items-center gap-2">
          <Users className="w-4 h-4 text-gray-500 dark:text-gray-400" />
          <span className="text-sm font-medium text-gray-800 dark:text-gray-200">
            {block.teamName}
          </span>
        </div>
        {block.description && (
          <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{block.description}</p>
        )}
      </div>

      {/* Speaker list */}
      {speakers.length > 0 && (
        <div className="px-3 py-1.5 flex flex-wrap gap-2 border-b border-gray-100 dark:border-gray-800">
          {speakers.map((s) => (
            <div key={s.id} className="flex items-center gap-1 text-xs">
              <span
                className="w-2 h-2 rounded-full flex-shrink-0"
                style={s.color ? { background: s.color } : undefined}
              />
              <span className="text-gray-700 dark:text-gray-300">{s.displayName}</span>
              {s.stance && (
                <span className="text-gray-400 dark:text-gray-500 italic">{s.stance}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Entry timeline */}
      <div className="px-3 py-2 space-y-1">
        {entries.map((entry, i) => (
          <EntryRow
            key={(entry.lineIndex as number | undefined) ?? i}
            entry={entry}
            speakers={speakers}
          />
        ))}
        {entries.length === 0 && (
          <p className="text-xs text-gray-400 dark:text-gray-500 italic">No entries</p>
        )}
      </div>
    </div>
  )
}
