import * as Tooltip from '@radix-ui/react-tooltip'
import { Bot, TreePine } from 'lucide-react'

interface SessionBadgesProps {
  vimMode: string | null | undefined
  agentName: string | null | undefined
  /** Optional description/activity text for the agent tooltip. */
  agentContext: string | null | undefined
  outputStyle: string | null | undefined
  worktreeName: string | null | undefined
  worktreePath: string | null | undefined
  worktreeBranch: string | null | undefined
  worktreeOriginalCwd: string | null | undefined
  worktreeOriginalBranch: string | null | undefined
}

const PILL =
  'inline-flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors duration-200 cursor-default'

const STYLE = {
  vim: `${PILL} font-mono bg-violet-100 text-violet-700 dark:bg-violet-900/40 dark:text-violet-300`,
  agent: `${PILL} bg-sky-100 text-sky-700 dark:bg-sky-900/40 dark:text-sky-300`,
  output: `${PILL} bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-400`,
  worktree: `${PILL} bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-300`,
} as const

const TIP =
  'bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg px-3 py-2 shadow-lg z-50 max-w-xs text-xs'
const ARROW = 'fill-gray-200 dark:fill-gray-700'

export function SessionBadges({
  vimMode,
  agentName,
  agentContext,
  outputStyle,
  worktreeName,
  worktreePath,
  worktreeBranch,
  worktreeOriginalCwd,
  worktreeOriginalBranch,
}: SessionBadgesProps) {
  const badges: React.ReactNode[] = []

  if (vimMode) {
    badges.push(
      <span key="vim" className={STYLE.vim}>
        VIM:{vimMode}
      </span>,
    )
  }

  if (agentName) {
    badges.push(
      <Tooltip.Root key="agent">
        <Tooltip.Trigger asChild>
          <span className={STYLE.agent}>
            <Bot className="h-2.5 w-2.5" />
            {agentName}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content className={TIP} sideOffset={5}>
            <p className="font-medium text-gray-900 dark:text-gray-100">Subagent: {agentName}</p>
            <p className="text-gray-500 dark:text-gray-400 mt-0.5">
              {agentContext ? agentContext : 'This session is running as a dispatched subagent.'}
            </p>
            <Tooltip.Arrow className={ARROW} />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>,
    )
  }

  // "default" is noise — only surface non-default output styles
  if (outputStyle && outputStyle !== 'default') {
    badges.push(
      <span key="output" className={STYLE.output}>
        {outputStyle}
      </span>,
    )
  }

  if (worktreeName) {
    badges.push(
      <Tooltip.Root key="worktree">
        <Tooltip.Trigger asChild>
          <span className={STYLE.worktree}>
            <TreePine className="h-2.5 w-2.5" />
            {worktreeName}
          </span>
        </Tooltip.Trigger>
        <Tooltip.Portal>
          <Tooltip.Content className={TIP} sideOffset={5}>
            <p className="font-medium text-gray-900 dark:text-gray-100">Worktree: {worktreeName}</p>
            <div className="mt-1 space-y-0.5 text-gray-500 dark:text-gray-400">
              {worktreeBranch && (
                <p>
                  Branch: <span className="font-mono">{worktreeBranch}</span>
                </p>
              )}
              {worktreeOriginalBranch && (
                <p>
                  From: <span className="font-mono">{worktreeOriginalBranch}</span>
                </p>
              )}
              {worktreePath && (
                <p className="font-mono text-[10px] text-gray-400 dark:text-gray-500 break-all">
                  {worktreePath}
                </p>
              )}
              {worktreeOriginalCwd && worktreeOriginalCwd !== worktreePath && (
                <p className="font-mono text-[10px] text-gray-400 dark:text-gray-500 break-all">
                  Origin: {worktreeOriginalCwd}
                </p>
              )}
            </div>
            <Tooltip.Arrow className={ARROW} />
          </Tooltip.Content>
        </Tooltip.Portal>
      </Tooltip.Root>,
    )
  }

  if (badges.length === 0) return null

  return (
    <Tooltip.Provider delayDuration={200}>
      <div className="flex items-center gap-1 flex-wrap">{badges}</div>
    </Tooltip.Provider>
  )
}
