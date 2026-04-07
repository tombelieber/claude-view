import type { TranscriptSpeaker } from '../../types/generated/TranscriptSpeaker'
import { formatModelName } from '../../lib/format-model'
import { cn } from '../../lib/utils'

const DOT_COLOR_MAP: Record<string, string> = {
  blue: 'bg-blue-500',
  green: 'bg-green-500',
  yellow: 'bg-yellow-500',
  purple: 'bg-purple-500',
  red: 'bg-red-500',
  orange: 'bg-orange-500',
}

interface TranscriptHeaderProps {
  topic: string
  speakers: TranscriptSpeaker[]
}

export function TranscriptHeader({ topic, speakers }: TranscriptHeaderProps) {
  return (
    <div className="space-y-4 pb-4 border-b border-zinc-200 dark:border-zinc-700">
      <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100 leading-snug">
        {topic}
      </h2>
      {speakers.length > 0 && (
        <div className="flex flex-wrap gap-3">
          {speakers.map((s) => (
            <div key={s.id} className="flex items-center gap-2">
              <span
                className={cn(
                  'w-3 h-3 rounded-full shrink-0',
                  DOT_COLOR_MAP[s.color ?? ''] ?? 'bg-gray-400',
                )}
              />
              <div className="flex items-center gap-1.5">
                <span className="text-xs font-medium text-gray-900 dark:text-gray-100">
                  {s.displayName}
                </span>
                {s.model && (
                  <span className="text-xs px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500">
                    {formatModelName(s.model)}
                  </span>
                )}
                {s.stance && (
                  <span className="text-xs text-zinc-500 dark:text-zinc-400">{s.stance}</span>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
