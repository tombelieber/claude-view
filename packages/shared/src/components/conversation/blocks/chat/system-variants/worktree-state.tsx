import type { WorktreeState } from '../../../../../types/sidecar-protocol'
import { GitBranch } from 'lucide-react'

interface Props {
  data: WorktreeState
}

export function WorktreeStatePill({ data }: Props) {
  const session = data?.worktreeSession
  return (
    <div className="flex items-center gap-2 px-3 py-1 text-xs text-gray-500 dark:text-gray-400">
      <GitBranch className="w-3 h-3 flex-shrink-0" />
      <span className="truncate">
        {session?.worktreeName} ({session?.worktreeBranch})
      </span>
    </div>
  )
}
