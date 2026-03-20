import type { LiveSession } from '@claude-view/shared/types/generated'
import type { SessionInfo } from '../types/generated/SessionInfo'

export type SidebarSession = SessionInfo & {
  isActive: boolean
  isWatching: boolean
  isSidecarManaged: boolean
  liveData: LiveSession | null
}

/**
 * Pure mapper: merges history sessions with live session data.
 *
 * Sidecar-managed detection uses SSE LiveSession.control field — set by the
 * Rust server when the sidecar calls POST /api/live/sessions/:id/bind-control.
 * No frontend workarounds (localSidecarIds, polling) needed.
 */
export function toSidebarItems(
  history: SessionInfo[],
  liveSessions: LiveSession[],
): SidebarSession[] {
  const liveMap = new Map(liveSessions.map((s) => [s.id, s]))
  const historyIds = new Set(history.map((h) => h.id))

  // Enrich history sessions with live data
  const result: SidebarSession[] = history.map((h) => {
    const live = liveMap.get(h.id) ?? null
    const isLiveActive =
      live != null &&
      (live.status === 'working' || live.status === 'paused' || live.control != null)
    const isSidecarManaged = live?.control != null

    return {
      ...h,
      isActive: isLiveActive,
      isWatching: isLiveActive && !isSidecarManaged,
      isSidecarManaged,
      liveData: live,
    }
  })

  // Append active live sessions not yet in history (newly created, not yet indexed)
  for (const live of liveSessions) {
    if (historyIds.has(live.id)) continue
    const isSidecarManaged = live.control != null
    const isLiveActive = live.status === 'working' || live.status === 'paused' || isSidecarManaged
    if (!isLiveActive) continue

    result.push({
      id: live.id,
      project: live.project,
      projectPath: live.projectPath,
      displayName: live.projectDisplayName,
      filePath: live.filePath,
      modifiedAt: live.lastActivityAt ?? Math.floor(Date.now() / 1000),
      sizeBytes: 0,
      preview: live.lastUserMessage || live.currentActivity,
      lastMessage: live.lastUserMessage,
      slug: live.slug,
      gitBranch: live.gitBranch,
      filesTouched: [],
      skillsUsed: [],
      toolCounts: { read: 0, edit: 0, write: 0, bash: 0, glob: 0, grep: 0, other: 0 },
      messageCount: 0,
      turnCount: live.turnCount,
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
      isActive: true,
      isWatching: !isSidecarManaged,
      isSidecarManaged,
      liveData: live,
    } as SidebarSession)
  }

  return result
}
