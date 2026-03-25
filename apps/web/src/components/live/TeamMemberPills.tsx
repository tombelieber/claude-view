import * as Tooltip from '@radix-ui/react-tooltip'
import type { TeamMember } from '../../types/generated/TeamMember'

const PILL_CLASS =
  'inline-flex items-center gap-1.5 px-2 py-0.5 rounded-md text-xs font-medium border border-dashed border-zinc-300 dark:border-zinc-600 bg-zinc-50 dark:bg-zinc-800 text-zinc-600 dark:text-zinc-400 cursor-default'

const TOOLTIP_CONTENT_CLASS =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs'
const TOOLTIP_ARROW_CLASS = 'fill-gray-200 dark:fill-gray-700'

interface TeamMemberPillsProps {
  members: TeamMember[]
}

export function TeamMemberPills({ members }: TeamMemberPillsProps) {
  // Filter out the team-lead — they're the parent session itself
  const agents = members.filter((m) => m.agentType !== 'team-lead')
  if (agents.length === 0) return null

  const displayMembers = agents.slice(0, 4)
  const overflowMembers = agents.slice(4)

  return (
    <Tooltip.Provider delayDuration={200}>
      <div className="flex items-center gap-1.5 px-1 py-0.5 flex-wrap">
        {displayMembers.map((member) => (
          <MemberPill key={member.agentId} member={member} />
        ))}
        {overflowMembers.length > 0 && <OverflowPill members={overflowMembers} />}
      </div>
    </Tooltip.Provider>
  )
}

function MemberPill({ member }: { member: TeamMember }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span className={PILL_CLASS}>
          <span className="w-2 h-2 rounded-full bg-zinc-400 dark:bg-zinc-500 shrink-0" />
          <span className="truncate max-w-24">{member.name}</span>
          <span className="text-xs opacity-60">{member.agentType}</span>
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <div className="space-y-1">
            <p className="font-medium text-gray-900 dark:text-gray-100">{member.name}</p>
            <div className="flex items-center gap-2 text-gray-500 dark:text-gray-400">
              <span className="px-1 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-xs">
                {member.model}
              </span>
              <span className="text-xs">{member.agentType}</span>
            </div>
            {member.prompt && (
              <p className="text-gray-400 dark:text-gray-500 line-clamp-3 mt-1">{member.prompt}</p>
            )}
          </div>
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}

function OverflowPill({ members }: { members: TeamMember[] }) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        <span className={PILL_CLASS}>+{members.length} more</span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content className={TOOLTIP_CONTENT_CLASS} sideOffset={5}>
          <div className="space-y-1.5">
            {members.map((m) => (
              <div key={m.agentId} className="flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-zinc-400 dark:bg-zinc-500 shrink-0" />
                <span className="font-medium text-gray-900 dark:text-gray-100">{m.name}</span>
                <span className="text-gray-400 text-xs">{m.model}</span>
              </div>
            ))}
          </div>
          <Tooltip.Arrow className={TOOLTIP_ARROW_CLASS} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  )
}
