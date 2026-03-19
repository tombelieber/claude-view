export type StageStatus = 'locked' | 'running' | 'passed' | 'failed'

interface TeamMemberLike {
  name: string
  agentId: string
}

interface SessionLike {
  state: string
}

export function nodeStatusFromLiveSession(
  stageId: string,
  teamMembers: TeamMemberLike[],
  liveSessions: Map<string, SessionLike>,
  gateResults: Map<string, boolean>,
): StageStatus {
  const member = teamMembers.find((m) => m.name === stageId)
  if (!member) return 'locked'

  const session = liveSessions.get(member.agentId)
  if (!session) return 'locked'

  switch (session.state) {
    case 'active':
    case 'waiting_permission':
    case 'compacting':
      return 'running'
    case 'closed': {
      const gatePassed = gateResults.get(stageId)
      if (gatePassed === undefined) return 'passed' // no gate = always pass
      return gatePassed ? 'passed' : 'failed'
    }
    case 'error':
      return 'failed'
    case 'initializing':
    case 'waiting_input':
    default:
      return 'locked'
  }
}
