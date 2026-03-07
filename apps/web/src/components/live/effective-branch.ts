export interface EffectiveBranch {
  branch: string | null
  driftOrigin: string | null
  isWorktree: boolean
}

export function getEffectiveBranch(
  gitBranch: string | null,
  worktreeBranch: string | null,
  isWorktree: boolean,
): EffectiveBranch {
  const branch = worktreeBranch ?? gitBranch
  const driftOrigin = worktreeBranch && gitBranch && worktreeBranch !== gitBranch ? gitBranch : null
  return { branch, driftOrigin, isWorktree }
}
