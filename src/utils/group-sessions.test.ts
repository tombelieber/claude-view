// src/utils/group-sessions.test.ts
import { describe, it, expect } from 'vitest';
import { groupSessions, shouldDisableGrouping, type SessionInfo, type SessionGroup } from './group-sessions';
import type { ToolCounts } from '../types/generated/ToolCounts';

// Helper to create a test session
function makeSession(overrides: Partial<SessionInfo> = {}): SessionInfo {
  const defaultToolCounts: ToolCounts = {
    bash: 0,
    edit: 0,
    read: 0,
    write: 0,
  };

  return {
    id: 'test-id',
    project: 'test-project',
    projectPath: '/test/path',
    filePath: '/test/file.jsonl',
    modifiedAt: BigInt(Math.floor(Date.now() / 1000)),
    sizeBytes: BigInt(1024),
    preview: 'Test preview',
    lastMessage: 'Last message',
    filesTouched: [],
    skillsUsed: [],
    toolCounts: defaultToolCounts,
    messageCount: 10,
    turnCount: 5,
    isSidechain: false,
    deepIndexed: true,
    userPromptCount: 5,
    apiCallCount: 10,
    toolCallCount: 20,
    filesRead: ['file1.ts'],
    filesEdited: ['file2.ts'],
    filesReadCount: 1,
    filesEditedCount: 1,
    reeditedFilesCount: 0,
    durationSeconds: 600,
    commitCount: 0,
    thinkingBlockCount: 0,
    apiErrorCount: 0,
    compactionCount: 0,
    agentSpawnCount: 0,
    bashProgressCount: 0,
    hookProgressCount: 0,
    mcpProgressCount: 0,
    parseVersion: 1,
    ...overrides,
  };
}

describe('shouldDisableGrouping', () => {
  it('returns false for <= 500 sessions', () => {
    expect(shouldDisableGrouping(0)).toBe(false);
    expect(shouldDisableGrouping(250)).toBe(false);
    expect(shouldDisableGrouping(500)).toBe(false);
  });

  it('returns true for > 500 sessions', () => {
    expect(shouldDisableGrouping(501)).toBe(true);
    expect(shouldDisableGrouping(1000)).toBe(true);
  });
});

describe('groupSessions', () => {
  describe('none grouping', () => {
    it('returns empty array when groupBy is none', () => {
      const sessions = [makeSession(), makeSession()];
      const groups = groupSessions(sessions, 'none');
      expect(groups).toEqual([]);
    });
  });

  describe('branch grouping', () => {
    it('groups sessions by branch', () => {
      const sessions = [
        makeSession({ id: 's1', gitBranch: 'main', filesEditedCount: 5, commitCount: 2 }),
        makeSession({ id: 's2', gitBranch: 'feature/auth', filesEditedCount: 10, commitCount: 3 }),
        makeSession({ id: 's3', gitBranch: 'main', filesEditedCount: 3, commitCount: 1 }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups).toHaveLength(2);

      // Find the "main" group
      const mainGroup = groups.find(g => g.key === 'main');
      expect(mainGroup).toBeDefined();
      expect(mainGroup!.sessions).toHaveLength(2);
      expect(mainGroup!.stats.sessionCount).toBe(2);
      expect(mainGroup!.stats.totalFiles).toBe(8); // 5 + 3
      expect(mainGroup!.stats.totalCommits).toBe(3); // 2 + 1

      // Find the "feature/auth" group
      const featureGroup = groups.find(g => g.key === 'feature/auth');
      expect(featureGroup).toBeDefined();
      expect(featureGroup!.sessions).toHaveLength(1);
      expect(featureGroup!.stats.sessionCount).toBe(1);
      expect(featureGroup!.stats.totalFiles).toBe(10);
    });

    it('groups sessions without branch into "(no branch)"', () => {
      const sessions = [
        makeSession({ id: 's1', gitBranch: 'main' }),
        makeSession({ id: 's2', gitBranch: undefined }),
        makeSession({ id: 's3', gitBranch: null }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups).toHaveLength(2);
      const noBranchGroup = groups.find(g => g.key === '(no branch)');
      expect(noBranchGroup).toBeDefined();
      expect(noBranchGroup!.sessions).toHaveLength(2);
    });

    it('sorts branches alphabetically with (no branch) last', () => {
      const sessions = [
        makeSession({ id: 's1', gitBranch: 'zebra' }),
        makeSession({ id: 's2', gitBranch: 'apple' }),
        makeSession({ id: 's3', gitBranch: null }),
        makeSession({ id: 's4', gitBranch: 'main' }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups.map(g => g.key)).toEqual(['apple', 'main', 'zebra', '(no branch)']);
    });
  });

  describe('project grouping', () => {
    it('groups sessions by project', () => {
      const sessions = [
        makeSession({ id: 's1', project: 'project-a', filesEditedCount: 5 }),
        makeSession({ id: 's2', project: 'project-b', filesEditedCount: 10 }),
        makeSession({ id: 's3', project: 'project-a', filesEditedCount: 3 }),
      ];

      const groups = groupSessions(sessions, 'project');

      expect(groups).toHaveLength(2);

      const projectA = groups.find(g => g.key === 'project-a');
      expect(projectA).toBeDefined();
      expect(projectA!.sessions).toHaveLength(2);
      expect(projectA!.stats.totalFiles).toBe(8);
    });

    it('sorts projects alphabetically', () => {
      const sessions = [
        makeSession({ id: 's1', project: 'zebra' }),
        makeSession({ id: 's2', project: 'apple' }),
        makeSession({ id: 's3', project: 'main' }),
      ];

      const groups = groupSessions(sessions, 'project');

      expect(groups.map(g => g.key)).toEqual(['apple', 'main', 'zebra']);
    });
  });

  describe('model grouping', () => {
    it('groups sessions by model', () => {
      const sessions = [
        makeSession({ id: 's1', primaryModel: 'claude-opus-4', filesEditedCount: 5 }),
        makeSession({ id: 's2', primaryModel: 'claude-sonnet-4', filesEditedCount: 10 }),
        makeSession({ id: 's3', primaryModel: 'claude-opus-4', filesEditedCount: 3 }),
      ];

      const groups = groupSessions(sessions, 'model');

      expect(groups).toHaveLength(2);

      const opusGroup = groups.find(g => g.key === 'claude-opus-4');
      expect(opusGroup).toBeDefined();
      expect(opusGroup!.sessions).toHaveLength(2);
      expect(opusGroup!.stats.totalFiles).toBe(8);
    });

    it('groups sessions without model into "(unknown model)"', () => {
      const sessions = [
        makeSession({ id: 's1', primaryModel: 'claude-opus-4' }),
        makeSession({ id: 's2', primaryModel: undefined }),
        makeSession({ id: 's3', primaryModel: null }),
      ];

      const groups = groupSessions(sessions, 'model');

      expect(groups).toHaveLength(2);
      const unknownGroup = groups.find(g => g.key === '(unknown model)');
      expect(unknownGroup).toBeDefined();
      expect(unknownGroup!.sessions).toHaveLength(2);
    });
  });

  describe('day grouping', () => {
    it('groups sessions by day', () => {
      const day1 = new Date('2026-01-15T10:00:00Z');
      const day2 = new Date('2026-01-16T10:00:00Z');

      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(day1.getTime() / 1000)), filesEditedCount: 5 }),
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(day2.getTime() / 1000)), filesEditedCount: 10 }),
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(day1.getTime() / 1000) + 3600), filesEditedCount: 3 }), // Same day, 1 hour later
      ];

      const groups = groupSessions(sessions, 'day');

      expect(groups).toHaveLength(2);

      const day1Group = groups.find(g => g.key === '2026-01-15');
      expect(day1Group).toBeDefined();
      expect(day1Group!.sessions).toHaveLength(2);
      expect(day1Group!.stats.totalFiles).toBe(8);
    });

    it('sorts days in descending order (most recent first)', () => {
      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(new Date('2026-01-10').getTime() / 1000)) }),
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(new Date('2026-01-15').getTime() / 1000)) }),
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(new Date('2026-01-12').getTime() / 1000)) }),
      ];

      const groups = groupSessions(sessions, 'day');

      expect(groups.map(g => g.key)).toEqual(['2026-01-15', '2026-01-12', '2026-01-10']);
    });
  });

  describe('week grouping', () => {
    it('groups sessions by week (Monday start)', () => {
      // Jan 13, 2026 is a Tuesday - week starts on Monday Jan 12
      const tue = new Date('2026-01-13T10:00:00Z');
      // Jan 14, 2026 is a Wednesday - same week
      const wed = new Date('2026-01-14T10:00:00Z');
      // Jan 20, 2026 is a Tuesday - next week (starts Monday Jan 19)
      const nextTue = new Date('2026-01-20T10:00:00Z');

      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(tue.getTime() / 1000)), filesEditedCount: 5 }),
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(wed.getTime() / 1000)), filesEditedCount: 10 }),
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(nextTue.getTime() / 1000)), filesEditedCount: 3 }),
      ];

      const groups = groupSessions(sessions, 'week');

      expect(groups).toHaveLength(2);

      // Week of Jan 12, 2026
      const week1 = groups.find(g => g.key === '2026-01-12');
      expect(week1).toBeDefined();
      expect(week1!.sessions).toHaveLength(2);
      expect(week1!.stats.totalFiles).toBe(15);
    });

    it('sorts weeks in descending order', () => {
      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(new Date('2026-01-13').getTime() / 1000)) }), // Week of Jan 12
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(new Date('2026-01-27').getTime() / 1000)) }), // Week of Jan 26
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(new Date('2026-01-20').getTime() / 1000)) }), // Week of Jan 19
      ];

      const groups = groupSessions(sessions, 'week');

      expect(groups.map(g => g.key)).toEqual(['2026-01-26', '2026-01-19', '2026-01-12']);
    });
  });

  describe('month grouping', () => {
    it('groups sessions by month', () => {
      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(new Date('2026-01-05').getTime() / 1000)), filesEditedCount: 5 }),
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(new Date('2026-02-10').getTime() / 1000)), filesEditedCount: 10 }),
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(new Date('2026-01-25').getTime() / 1000)), filesEditedCount: 3 }),
      ];

      const groups = groupSessions(sessions, 'month');

      expect(groups).toHaveLength(2);

      const jan = groups.find(g => g.key === '2026-01');
      expect(jan).toBeDefined();
      expect(jan!.sessions).toHaveLength(2);
      expect(jan!.stats.totalFiles).toBe(8);
    });

    it('sorts months in descending order', () => {
      const sessions = [
        makeSession({ id: 's1', modifiedAt: BigInt(Math.floor(new Date('2026-01-15').getTime() / 1000)) }),
        makeSession({ id: 's2', modifiedAt: BigInt(Math.floor(new Date('2026-03-15').getTime() / 1000)) }),
        makeSession({ id: 's3', modifiedAt: BigInt(Math.floor(new Date('2026-02-15').getTime() / 1000)) }),
      ];

      const groups = groupSessions(sessions, 'month');

      expect(groups.map(g => g.key)).toEqual(['2026-03', '2026-02', '2026-01']);
    });
  });

  describe('aggregate statistics', () => {
    it('computes correct token totals', () => {
      const sessions = [
        makeSession({
          id: 's1',
          gitBranch: 'main',
          totalInputTokens: BigInt(10000),
          totalOutputTokens: BigInt(5000),
        }),
        makeSession({
          id: 's2',
          gitBranch: 'main',
          totalInputTokens: BigInt(20000),
          totalOutputTokens: BigInt(10000),
        }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups).toHaveLength(1);
      expect(groups[0].stats.totalTokens).toBe(45000); // 10000 + 5000 + 20000 + 10000
    });

    it('handles missing token counts', () => {
      const sessions = [
        makeSession({
          id: 's1',
          gitBranch: 'main',
          totalInputTokens: undefined,
          totalOutputTokens: undefined,
        }),
        makeSession({
          id: 's2',
          gitBranch: 'main',
          totalInputTokens: BigInt(10000),
          totalOutputTokens: BigInt(5000),
        }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups).toHaveLength(1);
      expect(groups[0].stats.totalTokens).toBe(15000); // Only s2 contributes
    });

    it('sets expanded to true for all groups', () => {
      const sessions = [
        makeSession({ id: 's1', gitBranch: 'main' }),
        makeSession({ id: 's2', gitBranch: 'feature/auth' }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups.every(g => g.expanded)).toBe(true);
    });
  });

  describe('label formatting', () => {
    it('formats branch labels with stats', () => {
      const sessions = [
        makeSession({
          id: 's1',
          gitBranch: 'main',
          filesEditedCount: 23,
          totalInputTokens: BigInt(100000),
          totalOutputTokens: BigInt(45000),
          commitCount: 5,
        }),
      ];

      const groups = groupSessions(sessions, 'branch');

      expect(groups[0].label).toContain('main');
      expect(groups[0].label).toContain('1 sessions');
      expect(groups[0].label).toContain('145K tokens');
      expect(groups[0].label).toContain('23 files');
    });

    it('formats day labels with human-readable dates', () => {
      const sessions = [
        makeSession({
          id: 's1',
          modifiedAt: BigInt(Math.floor(new Date('2026-01-15T10:00:00Z').getTime() / 1000)),
        }),
      ];

      const groups = groupSessions(sessions, 'day');

      expect(groups[0].label).toMatch(/Jan 15, 2026/);
      expect(groups[0].label).toContain('1 sessions');
    });

    it('formats week labels with "Week of" prefix', () => {
      const sessions = [
        makeSession({
          id: 's1',
          modifiedAt: BigInt(Math.floor(new Date('2026-01-15T10:00:00Z').getTime() / 1000)),
        }),
      ];

      const groups = groupSessions(sessions, 'week');

      expect(groups[0].label).toContain('Week of');
      expect(groups[0].label).toMatch(/Jan 12, 2026/); // Monday of that week
    });

    it('formats month labels with full month name', () => {
      const sessions = [
        makeSession({
          id: 's1',
          modifiedAt: BigInt(Math.floor(new Date('2026-01-15T10:00:00Z').getTime() / 1000)),
        }),
      ];

      const groups = groupSessions(sessions, 'month');

      expect(groups[0].label).toContain('January 2026');
    });
  });
});
