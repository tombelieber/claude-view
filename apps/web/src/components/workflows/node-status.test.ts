import { describe, expect, it } from 'vitest'
import { nodeStatusFromLiveSession } from './node-status.js'

describe('nodeStatusFromLiveSession', () => {
  const makeTeamMember = (name: string, agentId: string) => ({ name, agentId })
  const makeLiveSession = (state: string) => ({ state })

  it('returns locked when no member found', () => {
    expect(nodeStatusFromLiveSession('unknown', [], new Map(), new Map())).toBe('locked')
  })

  it('returns locked when no session for agentId', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    expect(nodeStatusFromLiveSession('stage-1', members, new Map(), new Map())).toBe('locked')
  })

  it('returns running when session state is active', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('active')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('running')
  })

  it('returns running when session state is waiting_permission', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('waiting_permission')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('running')
  })

  it('returns running when session state is compacting', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('compacting')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('running')
  })

  it('returns passed when closed with no gate', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('closed')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('passed')
  })

  it('returns passed when closed with gate passed', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('closed')]])
    const gates = new Map([['stage-1', true]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, gates)).toBe('passed')
  })

  it('returns failed when closed with gate failed', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('closed')]])
    const gates = new Map([['stage-1', false]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, gates)).toBe('failed')
  })

  it('returns failed when session state is error', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('error')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('failed')
  })

  it('returns locked when session state is initializing', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('initializing')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('locked')
  })

  it('returns locked when session state is waiting_input', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('waiting_input')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('locked')
  })

  it('returns locked for unknown state', () => {
    const members = [makeTeamMember('stage-1', 'agent-1')]
    const sessions = new Map([['agent-1', makeLiveSession('unknown_state')]])
    expect(nodeStatusFromLiveSession('stage-1', members, sessions, new Map())).toBe('locked')
  })
})
