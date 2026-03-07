import { ChevronDown, Clock, FileText, GitCommitHorizontal, Wrench } from 'lucide-react'
import { useState } from 'react'
import type { ElementType, ReactNode } from 'react'
import type { ShareSessionMetadata } from '../types/message'
import { cn } from '../utils/cn'
import { formatDuration } from '../utils/format-duration'

interface SessionInfoPanelProps {
  metadata?: ShareSessionMetadata
  className?: string
}

function Section({
  title,
  icon: Icon,
  children,
}: {
  title: string
  icon: ElementType
  children: ReactNode
}) {
  const [open, setOpen] = useState(true)
  return (
    <div className="border-b border-gray-200 dark:border-gray-700 last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center justify-between w-full px-3 py-2 text-xs font-medium text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer"
      >
        <span className="flex items-center gap-1.5">
          <Icon className="w-3.5 h-3.5" />
          {title}
        </span>
        <ChevronDown className={cn('w-3 h-3 transition-transform', open && 'rotate-180')} />
      </button>
      {open && <div className="px-3 pb-2">{children}</div>}
    </div>
  )
}

function Stat({ label, value }: { label: string; value: string | number | undefined }) {
  if (value === undefined || value === null) return null
  return (
    <div className="flex justify-between text-xs py-0.5">
      <span className="text-gray-500 dark:text-gray-400">{label}</span>
      <span className="text-gray-900 dark:text-gray-100 font-medium">{value}</span>
    </div>
  )
}

export function SessionInfoPanel({ metadata, className }: SessionInfoPanelProps) {
  if (!metadata) return null

  const totalTokens = (metadata.totalInputTokens ?? 0) + (metadata.totalOutputTokens ?? 0)

  return (
    <div
      className={cn(
        'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden',
        className,
      )}
    >
      {/* Overview */}
      <Section title="Overview" icon={Clock}>
        <Stat
          label="Duration"
          value={metadata.durationSeconds ? formatDuration(metadata.durationSeconds) : undefined}
        />
        <Stat label="Model" value={metadata.primaryModel} />
        <Stat label="Tokens" value={totalTokens > 0 ? totalTokens.toLocaleString() : undefined} />
        <Stat label="Prompts" value={metadata.userPromptCount} />
        <Stat label="Tool calls" value={metadata.toolCallCount} />
      </Section>

      {/* Tools */}
      {(metadata.toolsUsed?.length ?? 0) > 0 && (
        <Section title="Tools" icon={Wrench}>
          {metadata.toolsUsed?.map((t) => (
            <Stat key={t.name} label={t.name} value={t.count} />
          ))}
        </Section>
      )}

      {/* Files */}
      {((metadata.filesRead?.length ?? 0) > 0 || (metadata.filesEdited?.length ?? 0) > 0) && (
        <Section title="Files" icon={FileText}>
          <Stat label="Read" value={metadata.filesRead?.length} />
          <Stat label="Edited" value={metadata.filesEdited?.length} />
        </Section>
      )}

      {/* Commits */}
      {(metadata.commits?.length ?? 0) > 0 && (
        <Section title="Commits" icon={GitCommitHorizontal}>
          {metadata.commits?.map((c) => (
            <div key={c.hash} className="text-xs py-0.5">
              <code className="text-gray-500 dark:text-gray-400">{c.hash.slice(0, 7)}</code>
              <span className="ml-1.5 text-gray-700 dark:text-gray-300">{c.message}</span>
            </div>
          ))}
        </Section>
      )}

      {/* Branch */}
      {metadata.gitBranch && (
        <div className="px-3 py-2 text-xs text-gray-500 dark:text-gray-400">
          Branch: <code className="text-gray-700 dark:text-gray-300">{metadata.gitBranch}</code>
        </div>
      )}
    </div>
  )
}
