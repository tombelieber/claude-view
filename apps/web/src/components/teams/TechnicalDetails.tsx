import type { TranscriptEntry } from '../../types/generated/TranscriptEntry'

interface TechnicalDetailsProps {
  entries: TranscriptEntry[]
}

export function TechnicalDetails({ entries }: TechnicalDetailsProps) {
  const relays = entries.filter(
    (e): e is Extract<TranscriptEntry, { kind: 'moderator_relay' }> => e.kind === 'moderator_relay',
  )
  const tasks = entries.filter(
    (e): e is Extract<TranscriptEntry, { kind: 'task_event' }> => e.kind === 'task_event',
  )
  const lifecycle = entries.filter(
    (e): e is Extract<TranscriptEntry, { kind: 'team_lifecycle' }> => e.kind === 'team_lifecycle',
  )
  const protocol = entries.filter(
    (e): e is Extract<TranscriptEntry, { kind: 'protocol' }> => e.kind === 'protocol',
  )

  const totalCount = relays.length + tasks.length + lifecycle.length + protocol.length
  if (totalCount === 0) return null

  return (
    <details className="mt-6 text-xs text-zinc-500 dark:text-zinc-400">
      <summary className="cursor-pointer hover:text-zinc-700 dark:hover:text-zinc-300 select-none">
        Show technical details ({totalCount} events)
      </summary>
      <div className="mt-3 space-y-3 border-l-2 border-zinc-200 dark:border-zinc-700 pl-3">
        {tasks.length > 0 && (
          <div>
            <div className="font-medium mb-1">Task Board</div>
            {tasks.map((t, i) => (
              <div key={i} className="flex gap-2 py-0.5">
                <span>{t.subject}</span>
                {t.status && (
                  <span className="px-1 rounded bg-zinc-100 dark:bg-zinc-800">{t.status}</span>
                )}
                {t.owner && <span className="text-zinc-400">({t.owner})</span>}
              </div>
            ))}
          </div>
        )}
        {relays.length > 0 && (
          <div>
            <div className="font-medium mb-1">Moderator Relays</div>
            {relays.map((r, i) => (
              <div key={i} className="py-0.5">
                <span className="text-indigo-500">→ {r.to}:</span>{' '}
                <span className="text-zinc-400">
                  {r.message.length > 100 ? `${r.message.slice(0, 100)}...` : r.message}
                </span>
              </div>
            ))}
          </div>
        )}
        {protocol.length > 0 && (
          <div>
            <div className="font-medium mb-1">Protocol ({protocol.length})</div>
            {protocol.map((p, i) => (
              <div key={i} className="py-0.5 text-zinc-400">
                <span>{p.teammateId}</span> <span className="italic">{p.msgType}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </details>
  )
}
