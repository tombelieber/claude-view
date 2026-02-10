import { bench, describe } from 'vitest';
import { groupSessions } from './group-sessions';
import type { SessionInfo } from '../types/generated/SessionInfo';
import type { ToolCounts } from '../types/generated/ToolCounts';

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

function generateSessions(count: number): SessionInfo[] {
  return Array.from({ length: count }, (_, i) => makeSession(i));
}

describe('groupSessions performance', () => {
  const sessions100 = generateSessions(100);
  const sessions500 = generateSessions(500);

  bench('group 100 sessions by branch', () => {
    groupSessions(sessions100, 'branch');
  });

  bench('group 500 sessions by branch', () => {
    groupSessions(sessions500, 'branch');
  });

  bench('group 100 sessions by model', () => {
    groupSessions(sessions100, 'model');
  });

  bench('group 500 sessions by model', () => {
    groupSessions(sessions500, 'model');
  });

  bench('group 100 sessions by week', () => {
    groupSessions(sessions100, 'week');
  });

  bench('group 500 sessions by week', () => {
    groupSessions(sessions500, 'week');
  });

  bench('group 500 sessions by month', () => {
    groupSessions(sessions500, 'month');
  });
});
