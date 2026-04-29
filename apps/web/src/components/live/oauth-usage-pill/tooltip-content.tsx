import { RefreshCw } from 'lucide-react'
import type { AuthIdentity } from '../../../hooks/use-auth-identity'
import type { OAuthUsage, UsageTier } from '../../../hooks/use-oauth-usage'
import { formatUpdatedAgo, isRedundantOrgName } from './format'
import { TierRow } from './tier-row'

interface ForceRefreshController {
  mutate: () => void
  isPending: boolean
  isError: boolean
  error?: { message?: string; name?: string } | null
}

interface UsageTooltipContentProps {
  data: OAuthUsage
  identity: AuthIdentity | undefined
  effectiveTiers: UsageTier[]
  dataUpdatedAt: number
  forceRefresh: ForceRefreshController
}

const SECTION_ORDER: Array<{ kind: string; label: string }> = [
  { kind: 'session', label: 'Session' },
  { kind: 'window', label: 'Weekly windows' },
  { kind: 'other', label: 'Additional tiers' },
  { kind: 'extra', label: 'Extra usage' },
]

function groupByKind(tiers: UsageTier[]): Map<string, UsageTier[]> {
  const groups = new Map<string, UsageTier[]>()
  for (const tier of tiers) {
    const key = (tier.kind as string | undefined) ?? 'window'
    const list = groups.get(key) ?? []
    list.push(tier)
    groups.set(key, list)
  }
  return groups
}

function Header({ data, identity }: { data: OAuthUsage; identity: AuthIdentity | undefined }) {
  return (
    <div className="mb-3 pb-2 border-b border-gray-200 dark:border-gray-700">
      <div className="flex items-center justify-between">
        <span className="text-xs font-semibold text-gray-600 dark:text-gray-400 uppercase tracking-wider">
          Usage
        </span>
        {data.plan && (
          <span className="text-xs px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">
            {data.plan}
          </span>
        )}
      </div>
      {identity?.hasAuth && identity.email && (
        <div className="mt-1.5 space-y-0.5">
          <div className="text-xs text-gray-500 dark:text-gray-400 truncate">{identity.email}</div>
          {identity.orgName && !isRedundantOrgName(identity.orgName, identity.email) && (
            <div className="text-xs text-gray-400 dark:text-gray-500 truncate">
              {identity.orgName}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function SectionHeading({ label }: { label: string }) {
  return (
    <div className="text-[10px] font-semibold uppercase tracking-wider text-gray-400 dark:text-gray-500">
      {label}
    </div>
  )
}

function Footer({
  dataUpdatedAt,
  forceRefresh,
}: {
  dataUpdatedAt: number
  forceRefresh: ForceRefreshController
}) {
  if (dataUpdatedAt <= 0) return null
  return (
    <div className="mt-3 pt-2 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
      <span className="text-xs text-gray-400 dark:text-gray-500">
        {forceRefresh.isError
          ? forceRefresh.error?.message || forceRefresh.error?.name || 'Refresh failed'
          : formatUpdatedAgo(dataUpdatedAt)}
      </span>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          forceRefresh.mutate()
        }}
        disabled={forceRefresh.isPending}
        className="p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 disabled:opacity-40 transition-colors"
        title="Refresh usage"
      >
        <RefreshCw className={`h-3 w-3 ${forceRefresh.isPending ? 'animate-spin' : ''}`} />
      </button>
    </div>
  )
}

export function UsageTooltipContent({
  data,
  identity,
  effectiveTiers,
  dataUpdatedAt,
  forceRefresh,
}: UsageTooltipContentProps) {
  const groups = groupByKind(effectiveTiers)

  // Render sections in stable order. Any unknown `kind` value sits with `window`
  // (the historical default) so a future codename doesn't disappear from the UI.
  const sectionsRendered: string[] = []
  const sections = SECTION_ORDER.flatMap(({ kind, label }) => {
    const tiers = groups.get(kind)
    if (!tiers || tiers.length === 0) return []
    sectionsRendered.push(kind)
    return [{ kind, label, tiers }]
  })
  for (const [kind, tiers] of groups) {
    if (sectionsRendered.includes(kind)) continue
    sections.push({ kind, label: kind, tiers })
  }

  return (
    <>
      <Header data={data} identity={identity} />
      <div className="space-y-3">
        {sections.map((section, idx) => (
          <div key={section.kind} className="space-y-2">
            {/* Section heading is suppressed for the very first group when it's
                the only one — keeps the legacy single-tier tooltip uncluttered. */}
            {(sections.length > 1 || idx > 0) && <SectionHeading label={section.label} />}
            <div className="space-y-3">
              {section.tiers.map((tier) => (
                <TierRow key={tier.id} tier={tier} />
              ))}
            </div>
          </div>
        ))}
      </div>
      <Footer dataUpdatedAt={dataUpdatedAt} forceRefresh={forceRefresh} />
    </>
  )
}
