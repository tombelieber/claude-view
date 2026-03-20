import type { LiveSession } from '@claude-view/shared/types/generated'
import type { SessionStatus } from '@claude-view/shared/types/generated/SessionStatus'
import { describe, expect, it } from 'vitest'
import type { SessionInfo } from '../../types/generated/SessionInfo'
import { toSidebarItems } from '../sidebar-mapper'

// ---------------------------------------------------------------------------
// Minimal fixtures — only fields the mapper actually reads
// ---------------------------------------------------------------------------

function makeSessionInfo(id: string, overrides?: Partial<SessionInfo>): SessionInfo {
  return {
    project: '',
    projectPath: '',
    displayName: '',
    filePath: '',
    modifiedAt: 1000,
    sizeBytes: 0,
    preview: '',
    lastMessage: '',
    filesTouched: [],
    skillsUsed: [],
    toolCounts: { read: 0, edit: 0, write: 0, bash: 0, glob: 0, grep: 0, other: 0 },
    messageCount: 0,
    turnCount: 0,
    isSidechain: false,
    deepIndexed: false,
    userPromptCount: 0,
    apiCallCount: 0,
    toolCallCount: 0,
    filesRead: [],
    filesEdited: [],
    filesReadCount: 0,
    filesEditedCount: 0,
    reeditedFilesCount: 0,
    durationSeconds: 0,
    commitCount: 0,
    thinkingBlockCount: 0,
    apiErrorCount: 0,
    compactionCount: 0,
    agentSpawnCount: 0,
    bashProgressCount: 0,
    hookProgressCount: 0,
    mcpProgressCount: 0,
    linesAdded: 0,
    linesRemoved: 0,
    locSource: 0,
    parseVersion: 0,
    correctionCount: 0,
    sameFileEditCount: 0,
    slug: null,
    ...overrides,
    id,
  } as SessionInfo
}

function makeLiveSession(id: string, overrides?: Partial<LiveSession>): LiveSession {
  return {
    project: '',
    projectDisplayName: '',
    projectPath: '',
    filePath: '',
    status: 'done' as SessionStatus,
    agentState: { group: 'idle', state: 'idle', label: '', confidence: 1 },
    gitBranch: null,
    worktreeBranch: null,
    isWorktree: false,
    effectiveBranch: null,
    pid: null,
    title: '',
    lastUserMessage: '',
    currentActivity: '',
    turnCount: 0,
    model: '',
    tokens: null,
    cost: null,
    cacheStatus: null,
    contextWindowTokens: 0,
    currentTurnStartedAt: null,
    lastTurnTaskSeconds: null,
    lastActivityAt: null,
    closedAt: null,
    subAgents: [],
    teamName: null,
    toolsUsed: [],
    progressItems: [],
    hookEvents: [],
    startedAt: null,
    editCount: 0,
    lastCacheHitAt: null,
    compactCount: 0,
    slug: null,
    control: null,
    exceeds200kTokens: false,
    ...overrides,
    id,
  } as unknown as LiveSession
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('toSidebarItems', () => {
  it('returns history sessions as-is when no live sessions exist', () => {
    const history = [makeSessionInfo('aaa')]
    const result = toSidebarItems(history, [])

    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('aaa')
    expect(result[0].isActive).toBe(false)
    expect(result[0].isWatching).toBe(false)
    expect(result[0].isSidecarManaged).toBe(false)
    expect(result[0].liveData).toBeNull()
  })

  it('marks session as active + watching when live but NOT sidecar-managed', () => {
    const history = [makeSessionInfo('bbb')]
    const live = [makeLiveSession('bbb', { status: 'working' })]
    const result = toSidebarItems(history, live)

    expect(result[0].isActive).toBe(true)
    expect(result[0].isWatching).toBe(true)
    expect(result[0].isSidecarManaged).toBe(false)
    expect(result[0].liveData).toBe(live[0])
  })

  it('marks session as active + sidecar-managed when control binding exists', () => {
    const history = [makeSessionInfo('ccc')]
    const live = [
      makeLiveSession('ccc', {
        status: 'working',
        control: { controlId: 'ctrl-1', boundAt: 999 },
      }),
    ]
    const result = toSidebarItems(history, live)

    expect(result[0].isActive).toBe(true)
    expect(result[0].isWatching).toBe(false)
    expect(result[0].isSidecarManaged).toBe(true)
  })

  it('isWatching is false for done live sessions (not working/paused)', () => {
    const history = [makeSessionInfo('ddd')]
    const live = [makeLiveSession('ddd', { status: 'done' })]
    const result = toSidebarItems(history, live)

    // Idle session with no control → not active, not watching
    expect(result[0].isActive).toBe(false)
    expect(result[0].isWatching).toBe(false)
  })

  it('includes active live sessions not yet in history (newly created)', () => {
    const history = [makeSessionInfo('aaa')]
    const live = [makeLiveSession('zzz', { status: 'working', title: 'New session' })]
    const result = toSidebarItems(history, live)

    // Live-only active sessions are appended — they exist but aren't indexed yet
    expect(result).toHaveLength(2)
    expect(result[0].id).toBe('aaa')
    expect(result[1].id).toBe('zzz')
    expect(result[1].isActive).toBe(true)
    expect(result[1].isWatching).toBe(true)
  })

  it('does NOT include done live sessions not in history', () => {
    const history = [makeSessionInfo('aaa')]
    const live = [makeLiveSession('zzz', { status: 'done' })]
    const result = toSidebarItems(history, live)

    // Done sessions without history entry are not interesting — skip
    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('aaa')
  })

  it('preserves all original SessionInfo fields', () => {
    const history = [makeSessionInfo('eee', { slug: 'my-session', preview: 'hello world' })]
    const result = toSidebarItems(history, [])

    expect(result[0].slug).toBe('my-session')
    expect(result[0].preview).toBe('hello world')
  })

  it('handles multiple sessions with mixed states', () => {
    const history = [
      makeSessionInfo('h1', { modifiedAt: 3000 }),
      makeSessionInfo('h2', { modifiedAt: 2000 }),
      makeSessionInfo('h3', { modifiedAt: 1000 }),
    ]
    const live = [
      makeLiveSession('h1', { status: 'working' }),
      makeLiveSession('h3', {
        status: 'paused',
        control: { controlId: 'c1', boundAt: 500 },
      }),
    ]
    const result = toSidebarItems(history, live)

    // h1: live + no control → watching
    expect(result[0].isActive).toBe(true)
    expect(result[0].isWatching).toBe(true)
    expect(result[0].isSidecarManaged).toBe(false)

    // h2: not live → history only
    expect(result[1].isActive).toBe(false)
    expect(result[1].isWatching).toBe(false)

    // h3: live + control → sidecar-managed
    expect(result[2].isActive).toBe(true)
    expect(result[2].isWatching).toBe(false)
    expect(result[2].isSidecarManaged).toBe(true)
  })

  it('SSE control field marks sidecar-managed sessions (systematic path)', () => {
    // After sidecar→Rust server bind-control notification, SSE has control set
    const history = [makeSessionInfo('aaa'), makeSessionInfo('bbb')]
    const live = [
      makeLiveSession('aaa', {
        status: 'paused',
        control: { controlId: 'ctrl-1', boundAt: 100 },
      }),
      makeLiveSession('bbb', { status: 'working' }),
    ]
    const result = toSidebarItems(history, live)

    // aaa: live + control → sidecar-managed, NOT watching
    expect(result[0].isActive).toBe(true)
    expect(result[0].isSidecarManaged).toBe(true)
    expect(result[0].isWatching).toBe(false)

    // bbb: live + no control → watching
    expect(result[1].isActive).toBe(true)
    expect(result[1].isSidecarManaged).toBe(false)
    expect(result[1].isWatching).toBe(true)
  })

  it('live-only session with control is sidecar-managed (not yet indexed)', () => {
    const history: SessionInfo[] = []
    const live = [
      makeLiveSession('new-sidecar', {
        status: 'working',
        control: { controlId: 'ctrl-2', boundAt: 200 },
      }),
    ]
    const result = toSidebarItems(history, live)

    expect(result).toHaveLength(1)
    expect(result[0].id).toBe('new-sidecar')
    expect(result[0].isActive).toBe(true)
    expect(result[0].isSidecarManaged).toBe(true)
    expect(result[0].isWatching).toBe(false)
  })

  // --- Focus safety: reference stability ---

  it('returns same-length array for identical inputs (no phantom growth)', () => {
    const history = [makeSessionInfo('a1'), makeSessionInfo('a2')]
    const live = [makeLiveSession('a1', { status: 'working' })]

    const result1 = toSidebarItems(history, live)
    const result2 = toSidebarItems(history, live)

    expect(result1).toHaveLength(result2.length)
    expect(result1.map((s) => s.id)).toEqual(result2.map((s) => s.id))
  })

  it('is a pure function — same inputs produce structurally equal outputs', () => {
    const history = [makeSessionInfo('p1')]
    const live = [makeLiveSession('p1', { status: 'working' })]

    const r1 = toSidebarItems(history, live)
    const r2 = toSidebarItems(history, live)

    expect(r1[0].isActive).toBe(r2[0].isActive)
    expect(r1[0].isWatching).toBe(r2[0].isWatching)
    expect(r1[0].isSidecarManaged).toBe(r2[0].isSidecarManaged)
  })

  // --- Edge: empty inputs ---

  it('handles empty history and empty live', () => {
    const result = toSidebarItems([], [])
    expect(result).toHaveLength(0)
  })

  it('handles empty history with live sessions (all appended)', () => {
    const live = [
      makeLiveSession('x1', { status: 'working' }),
      makeLiveSession('x2', { status: 'paused' }),
    ]
    const result = toSidebarItems([], live)

    expect(result).toHaveLength(2)
    expect(result[0].isActive).toBe(true)
    expect(result[1].isActive).toBe(true)
  })
})
