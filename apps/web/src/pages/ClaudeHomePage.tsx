import { FileText, FolderKey, LockKeyhole, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Virtuoso } from 'react-virtuoso'
import { useClaudeHomeEntries } from '../hooks/use-workflows'
import { cn } from '../lib/utils'
import type { ClaudeHomeEntry } from '../types/generated/ClaudeHomeEntry'

const ROW_GRID = 'grid grid-cols-[minmax(220px,1.4fr)_140px_110px_140px] gap-4'

function formatBytes(bytes: number): string {
  return new Intl.NumberFormat(undefined, {
    notation: bytes >= 10_000 ? 'compact' : 'standard',
  }).format(bytes)
}

function formatDate(value: number | null): string {
  if (!value) return 'Unknown'
  return new Intl.DateTimeFormat(undefined, {
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  }).format(new Date(Number(value)))
}

function EntryRow({ entry }: { entry: ClaudeHomeEntry }) {
  return (
    <div className={cn(ROW_GRID, 'border-b border-gray-200 px-4 py-3 dark:border-gray-800')}>
      <div className="min-w-0">
        <div className="flex items-center gap-2">
          {entry.metadataOnly ? (
            <LockKeyhole className="h-4 w-4 shrink-0 text-amber-500" />
          ) : entry.isDirectory ? (
            <FolderKey className="h-4 w-4 shrink-0 text-gray-500" />
          ) : (
            <FileText className="h-4 w-4 shrink-0 text-gray-500" />
          )}
          <span className="truncate text-sm font-medium text-gray-950 dark:text-white">
            {entry.name}
          </span>
        </div>
        <div className="mt-1 truncate text-xs text-gray-500">{entry.relativePath}</div>
        {entry.preview && (
          <pre className="mt-3 max-h-32 overflow-auto whitespace-pre-wrap rounded-md bg-gray-50 p-3 text-xs leading-relaxed text-gray-700 dark:bg-gray-900 dark:text-gray-300">
            {entry.preview}
          </pre>
        )}
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">
        <span className="rounded-md border border-gray-200 px-2 py-1 dark:border-gray-800">
          {entry.kind}
        </span>
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">
        <div>{formatBytes(Number(entry.sizeBytes))} B</div>
        {entry.itemCount > 0 && <div className="mt-1 text-gray-400">{entry.itemCount} items</div>}
      </div>
      <div className="text-xs text-gray-600 dark:text-gray-300">{formatDate(entry.modifiedAt)}</div>
    </div>
  )
}

export function ClaudeHomePage() {
  const { data: entries = [], isLoading, isError } = useClaudeHomeEntries()
  const [query, setQuery] = useState('')
  const [kind, setKind] = useState('all')

  const kinds = useMemo(
    () => Array.from(new Set(entries.map((entry) => entry.kind))).sort(),
    [entries],
  )
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    return entries.filter((entry) => {
      if (kind !== 'all' && entry.kind !== kind) return false
      if (!normalized) return true
      return [entry.name, entry.relativePath, entry.kind, entry.preview]
        .filter(Boolean)
        .some((value) => String(value).toLowerCase().includes(normalized))
    })
  }, [entries, kind, query])

  const metadataOnly = entries.filter((entry) => entry.metadataOnly).length
  const previewable = entries.length - metadataOnly

  return (
    <div className="flex h-full flex-col overflow-hidden bg-gray-50 dark:bg-black">
      <div className="shrink-0 border-b border-gray-200 bg-white px-8 py-6 dark:border-gray-800 dark:bg-gray-950">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h1 className="text-2xl font-semibold tracking-tight text-gray-950 dark:text-white">
              Claude Home
            </h1>
            <p className="mt-1 text-sm text-gray-500 dark:text-gray-400">
              Read-only metadata and safe previews from ~/.claude.
            </p>
          </div>
          <div className="grid grid-cols-3 gap-4 text-right">
            <div>
              <div className="text-lg font-semibold text-gray-950 dark:text-white">
                {entries.length}
              </div>
              <div className="text-xs text-gray-500">entries</div>
            </div>
            <div>
              <div className="text-lg font-semibold text-gray-950 dark:text-white">
                {previewable}
              </div>
              <div className="text-xs text-gray-500">previewed</div>
            </div>
            <div>
              <div className="text-lg font-semibold text-gray-950 dark:text-white">
                {metadataOnly}
              </div>
              <div className="text-xs text-gray-500">metadata-only</div>
            </div>
          </div>
        </div>
      </div>

      <div className="shrink-0 px-8 pt-6">
        <div className="flex flex-wrap gap-2">
          <label className="relative min-w-[260px] flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search Claude home entries"
              className="h-9 w-full rounded-md border border-gray-200 bg-white pl-9 pr-3 text-sm outline-none focus:border-blue-400 focus:ring-2 focus:ring-blue-100 dark:border-gray-800 dark:bg-gray-950 dark:text-white dark:focus:ring-blue-950"
            />
          </label>
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value)}
            className="h-9 rounded-md border border-gray-200 bg-white px-3 text-sm dark:border-gray-800 dark:bg-gray-950 dark:text-white"
          >
            <option value="all">All areas</option>
            {kinds.map((entryKind) => (
              <option key={entryKind} value={entryKind}>
                {entryKind}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-4 px-8 pb-6 pt-4">
        <div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-lg border border-gray-200 bg-white dark:border-gray-800 dark:bg-gray-950">
          <div
            className={cn(
              ROW_GRID,
              'shrink-0 border-b border-gray-200 px-4 py-2 text-xs font-medium uppercase tracking-wide text-gray-500 dark:border-gray-800',
            )}
          >
            <div>Name</div>
            <div>Area</div>
            <div>Size</div>
            <div>Modified</div>
          </div>
          {isLoading ? (
            <div className="flex h-40 items-center justify-center text-sm text-gray-500">
              Loading Claude home...
            </div>
          ) : isError ? (
            <div className="flex h-40 items-center justify-center px-4 text-center text-sm text-gray-500">
              Could not read ~/.claude metadata.
            </div>
          ) : filtered.length > 0 ? (
            <Virtuoso
              className="min-h-0 flex-1"
              data={filtered}
              computeItemKey={(_, entry) => `${entry.kind}:${entry.relativePath}`}
              itemContent={(_, entry) => <EntryRow entry={entry} />}
            />
          ) : (
            <div className="flex flex-1 items-center justify-center px-4 py-16 text-sm text-gray-500">
              No Claude home entries found.
            </div>
          )}
        </div>

        <div
          className={cn(
            'shrink-0 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-xs text-amber-800',
            'dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-200',
          )}
        >
          session-env, shell-snapshots, and file-history are intentionally metadata-only.
        </div>
      </div>
    </div>
  )
}
