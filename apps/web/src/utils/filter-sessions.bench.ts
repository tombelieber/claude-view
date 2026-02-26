import { bench, describe } from 'vitest';
import type { SessionInfo } from '../types/generated/SessionInfo';
import type { ToolCounts } from '../types/generated/ToolCounts';
import type { SessionFilters } from '../hooks/use-session-filters';
import { DEFAULT_FILTERS } from '../hooks/use-session-filters';

function makeSession(i: number): SessionInfo {
  const defaultToolCounts: ToolCounts = { bash: 0, edit: 0, read: 0, write: 0 };
  const branches = ['main', 'feature/auth', 'feature/ui', 'fix/bug-123', 'dev', 'staging'];
  const models = ['claude-opus-4', 'claude-sonnet-4', 'claude-haiku-4'];
  const baseTime = Math.floor(Date.now() / 1000) - i * 3600;

  return {
    id: `session-${i}`,
    project: `project-${i % 5}`,
    projectPath: `/test/project-${i % 5}`,
    filePath: `/test/file-${i}.jsonl`,
    modifiedAt: BigInt(baseTime),
    sizeBytes: BigInt(1024 * (i + 1)),
    preview: `Session preview ${i}`,
    lastMessage: `Last message ${i}`,
    filesTouched: [`file${i}.ts`, `file${i + 1}.ts`],
    skillsUsed: i % 3 === 0 ? ['skill-a'] : [],
    toolCounts: defaultToolCounts,
    messageCount: 10 + i,
    turnCount: 5 + i,
    isSidechain: false,
    deepIndexed: true,
    userPromptCount: 5 + (i % 20),
    apiCallCount: 10 + i,
    toolCallCount: 20 + i,
    filesRead: [`file${i}.ts`],
    filesEdited: [`file${i + 1}.ts`],
    filesReadCount: 1,
    filesEditedCount: 1 + (i % 5),
    reeditedFilesCount: i % 4 === 0 ? 1 : 0,
    durationSeconds: 300 + i * 60,
    commitCount: i % 2,
    gitBranch: branches[i % branches.length],
    primaryModel: models[i % models.length],
    totalInputTokens: BigInt(5000 + i * 100),
    totalOutputTokens: BigInt(2000 + i * 50),
    thinkingBlockCount: 0,
    apiErrorCount: 0,
    compactionCount: 0,
    agentSpawnCount: 0,
    bashProgressCount: 0,
    hookProgressCount: 0,
    mcpProgressCount: 0,
    linesAdded: i * 10,
    linesRemoved: i * 3,
    locSource: 100 + i,
    parseVersion: 1,
  };
}

function filterSessions(sessions: SessionInfo[], filters: SessionFilters): SessionInfo[] {
  return sessions.filter(s => {
    if (filters.branches.length > 0) {
      if (!s.gitBranch || !filters.branches.includes(s.gitBranch)) return false;
    }
    if (filters.models.length > 0) {
      if (!s.primaryModel || !filters.models.includes(s.primaryModel)) return false;
    }
    if (filters.hasCommits === 'yes' && (s.commitCount ?? 0) === 0) return false;
    if (filters.hasCommits === 'no' && (s.commitCount ?? 0) > 0) return false;
    if (filters.hasSkills === 'yes' && (s.skillsUsed ?? []).length === 0) return false;
    if (filters.hasSkills === 'no' && (s.skillsUsed ?? []).length > 0) return false;
    if (filters.minDuration !== null && (s.durationSeconds ?? 0) < filters.minDuration) return false;
    if (filters.minFiles !== null && (s.filesEditedCount ?? 0) < filters.minFiles) return false;
    if (filters.minTokens !== null) {
      const totalTokens = Number((s.totalInputTokens ?? 0n) + (s.totalOutputTokens ?? 0n));
      if (totalTokens < filters.minTokens) return false;
    }
    if (filters.highReedit === true) {
      const filesEdited = s.filesEditedCount ?? 0;
      const reeditedFiles = s.reeditedFilesCount ?? 0;
      const reeditRate = filesEdited > 0 ? reeditedFiles / filesEdited : 0;
      if (reeditRate <= 0.2) return false;
    }
    return true;
  });
}

function generateSessions(count: number): SessionInfo[] {
  return Array.from({ length: count }, (_, i) => makeSession(i));
}

describe('filterSessions performance', () => {
  const sessions500 = generateSessions(500);
  const sessions1000 = generateSessions(1000);

  const noFilters = DEFAULT_FILTERS;

  const branchFilter: SessionFilters = {
    ...DEFAULT_FILTERS,
    branches: ['main', 'feature/auth'],
  };

  const heavyFilter: SessionFilters = {
    ...DEFAULT_FILTERS,
    branches: ['main'],
    models: ['claude-opus-4'],
    hasCommits: 'yes',
    minDuration: 1800,
    minTokens: 50000,
    highReedit: true,
  };

  bench('filter 500 sessions — no filters', () => {
    filterSessions(sessions500, noFilters);
  });

  bench('filter 500 sessions — branch filter', () => {
    filterSessions(sessions500, branchFilter);
  });

  bench('filter 500 sessions — all filters active', () => {
    filterSessions(sessions500, heavyFilter);
  });

  bench('filter 1000 sessions — no filters', () => {
    filterSessions(sessions1000, noFilters);
  });

  bench('filter 1000 sessions — all filters active', () => {
    filterSessions(sessions1000, heavyFilter);
  });
});
