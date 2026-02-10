// src/utils/group-sessions.ts
/**
 * Client-side session grouping utilities for SessionToolbar.
 *
 * Groups sessions by various dimensions (branch, project, model, time period)
 * and computes aggregate statistics for each group.
 */

import type { SessionInfo } from '../types/generated/SessionInfo';

export type GroupBy = 'none' | 'branch' | 'project' | 'model' | 'day' | 'week' | 'month';

export interface SessionGroup {
  /** Group identifier (branch name, project name, model name, or date string) */
  key: string;
  /** Human-readable group label */
  label: string;
  /** Sessions in this group */
  sessions: SessionInfo[];
  /** Aggregate statistics */
  stats: GroupStats;
  /** Whether the group is currently expanded (for UI state) */
  expanded: boolean;
}

export interface GroupStats {
  /** Total number of sessions in group */
  sessionCount: number;
  /** Total tokens (input + output) across all sessions */
  totalTokens: number;
  /** Total files edited across all sessions */
  totalFiles: number;
  /** Total lines added across all sessions (from commit data if available) */
  totalLinesAdded?: number;
  /** Total lines removed across all sessions (from commit data if available) */
  totalLinesRemoved?: number;
  /** Total commits across all sessions */
  totalCommits: number;
}

/**
 * Group sessions by the specified dimension.
 * Returns an array of groups, each containing sessions and aggregate stats.
 *
 * @param sessions - Array of sessions to group
 * @param groupBy - Grouping dimension
 * @returns Array of session groups with stats
 */
export function groupSessions(
  sessions: SessionInfo[],
  groupBy: GroupBy
): SessionGroup[] {
  if (groupBy === 'none') {
    return [];
  }

  // Group sessions by key
  const groups = new Map<string, SessionInfo[]>();

  for (const session of sessions) {
    const key = getGroupKey(session, groupBy);
    if (!groups.has(key)) {
      groups.set(key, []);
    }
    groups.get(key)!.push(session);
  }

  // Convert to SessionGroup array with stats
  const result: SessionGroup[] = [];

  for (const [key, groupSessions] of groups.entries()) {
    const stats = computeGroupStats(groupSessions);
    const label = formatGroupLabel(key, groupBy, stats);

    result.push({
      key,
      label,
      sessions: groupSessions,
      stats,
      expanded: true, // All groups start expanded
    });
  }

  // Sort groups by appropriate criteria
  return sortGroups(result, groupBy);
}

/** Maximum session count before grouping is disabled for performance */
export const MAX_GROUPABLE_SESSIONS = 500;

/**
 * Check if grouping should be disabled due to session count.
 * When total > 500, client-side grouping can cause UI lag.
 */
export function shouldDisableGrouping(totalSessions: number): boolean {
  return totalSessions > MAX_GROUPABLE_SESSIONS;
}

/**
 * Get the group key for a session based on grouping dimension.
 */
function getGroupKey(session: SessionInfo, groupBy: GroupBy): string {
  switch (groupBy) {
    case 'branch':
      return session.gitBranch || '(no branch)';

    case 'project':
      return session.project;

    case 'model':
      return session.primaryModel || '(unknown model)';

    case 'day': {
      const ts = Number(session.modifiedAt);
      if (ts <= 0) return '(unknown date)';
      const date = new Date(ts * 1000);
      // Use local date, not UTC — toISOString() shifts the date for east-of-UTC users
      const year = date.getFullYear();
      const month = String(date.getMonth() + 1).padStart(2, '0');
      const day = String(date.getDate()).padStart(2, '0');
      return `${year}-${month}-${day}`;
    }

    case 'week': {
      const ts = Number(session.modifiedAt);
      if (ts <= 0) return '(unknown date)';
      const date = new Date(ts * 1000);
      const startOfWeek = getStartOfWeek(date);
      const year = startOfWeek.getFullYear();
      const month = String(startOfWeek.getMonth() + 1).padStart(2, '0');
      const day = String(startOfWeek.getDate()).padStart(2, '0');
      return `${year}-${month}-${day}`;
    }

    case 'month': {
      const ts = Number(session.modifiedAt);
      if (ts <= 0) return '(unknown date)';
      const date = new Date(ts * 1000);
      return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`; // YYYY-MM
    }

    default:
      return 'unknown';
  }
}

/**
 * Get the start of week (Monday) for a given date.
 * Uses UTC to avoid timezone issues.
 */
function getStartOfWeek(date: Date): Date {
  const day = date.getUTCDay();
  const diff = date.getUTCDate() - day + (day === 0 ? -6 : 1); // Adjust for Sunday
  const monday = new Date(date);
  monday.setUTCDate(diff);
  monday.setUTCHours(0, 0, 0, 0);
  return monday;
}

/**
 * Format a human-readable label for a group.
 */
function formatGroupLabel(key: string, groupBy: GroupBy, stats: GroupStats): string {
  switch (groupBy) {
    case 'branch':
      return `${key} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files · ${formatLineChanges(stats)}`;

    case 'project':
      return `${key} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files`;

    case 'model':
      return `${key} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files`;

    case 'day': {
      const date = new Date(key);
      const formatted = date.toLocaleDateString('en-US', {
        weekday: 'short',
        year: 'numeric',
        month: 'short',
        day: 'numeric'
      });
      return `${formatted} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files`;
    }

    case 'week': {
      const date = new Date(key);
      const formatted = date.toLocaleDateString('en-US', {
        month: 'short',
        day: 'numeric',
        year: 'numeric'
      });
      return `Week of ${formatted} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files`;
    }

    case 'month': {
      const [year, month] = key.split('-');
      const date = new Date(parseInt(year), parseInt(month) - 1);
      const formatted = date.toLocaleDateString('en-US', {
        month: 'long',
        year: 'numeric'
      });
      return `${formatted} — ${stats.sessionCount} sessions · ${formatTokens(stats.totalTokens)} · ${stats.totalFiles} files`;
    }

    default:
      return key;
  }
}

/**
 * Format token count with K suffix (e.g., "145K tokens").
 */
function formatTokens(tokens: number): string {
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(0)}K tokens`;
  }
  return `${tokens} tokens`;
}

/**
 * Format line changes (e.g., "+1.2K / -340 lines").
 */
function formatLineChanges(stats: GroupStats): string {
  const added = stats.totalLinesAdded || 0;
  const removed = stats.totalLinesRemoved || 0;

  if (added === 0 && removed === 0) {
    return '';
  }

  const formatNum = (n: number) => {
    if (n >= 1000) {
      return `${(n / 1000).toFixed(1)}K`;
    }
    return String(n);
  };

  return `+${formatNum(added)} / -${formatNum(removed)} lines`;
}

/**
 * Compute aggregate statistics for a group of sessions.
 */
function computeGroupStats(sessions: SessionInfo[]): GroupStats {
  let totalTokens = 0;
  let totalFiles = 0;
  let totalCommits = 0;

  for (const session of sessions) {
    // Sum tokens (convert bigint to number)
    const inputTokens = session.totalInputTokens ? Number(session.totalInputTokens) : 0;
    const outputTokens = session.totalOutputTokens ? Number(session.totalOutputTokens) : 0;
    totalTokens += inputTokens + outputTokens;

    // Sum files edited
    totalFiles += session.filesEditedCount;

    // Sum commits
    totalCommits += session.commitCount;
  }

  return {
    sessionCount: sessions.length,
    totalTokens,
    totalFiles,
    totalCommits,
    // Note: totalLinesAdded and totalLinesRemoved would come from commit data,
    // which requires fetching commit details from the backend.
    // For MVP, we skip this and leave them undefined.
  };
}

/**
 * Sort groups by appropriate criteria.
 */
function sortGroups(groups: SessionGroup[], groupBy: GroupBy): SessionGroup[] {
  switch (groupBy) {
    case 'branch':
    case 'project':
    case 'model':
      // Sort alphabetically by key, but put "(no branch)" / "(unknown model)" last
      return groups.sort((a, b) => {
        if (a.key.startsWith('(') && !b.key.startsWith('(')) return 1;
        if (!a.key.startsWith('(') && b.key.startsWith('(')) return -1;
        return a.key.localeCompare(b.key);
      });

    case 'day':
    case 'week':
    case 'month':
      // Sort by date descending (most recent first)
      return groups.sort((a, b) => b.key.localeCompare(a.key));

    default:
      return groups;
  }
}
