/**
 * Additional fixture data for developer mode Storybook stories.
 * rawJson fixtures for detail panels + system block variant fixtures not in base fixtures.
 */
import type { AssistantBlock, SystemBlock, UserBlock } from '@claude-view/shared/types/blocks'
import type {
  AiTitle,
  CommandOutput,
  ElicitationComplete,
  FileHistorySnapshot,
  FilesSaved,
  HookEvent,
  Informational,
  LastPrompt,
  LocalCommand,
  QueueOperation,
  SessionInit,
  SessionStatus,
  StreamDelta,
  UnknownSdkEvent,
} from '@claude-view/shared/types/sidecar-protocol'
import { assistantBlocks, userBlocks } from './fixtures'

// ── Raw JSON Fixtures ───────────────────────────────────────────────────────

export const rawJsonFixtures = {
  withLineage: {
    parentUuid: 'abc12345-dead-beef-cafe-123456789012',
    logicalParentUuid: 'def98765-fade-bead-cafe-987654321098',
    isSidechain: false,
    agentId: 'agent_main',
    uuid: '11111111-2222-3333-4444-555555555555',
    messageId: 'msg_001',
    sessionId: 'session_001',
  } as Record<string, unknown>,

  withStopReason: {
    stopReason: 'end_turn',
    preventedContinuation: false,
    hasOutput: true,
  } as Record<string, unknown>,

  withStopReasonPrevented: {
    stopReason: 'max_tokens',
    preventedContinuation: true,
    hasOutput: false,
  } as Record<string, unknown>,

  withApiError: {
    apiError: {
      message: 'Rate limit exceeded. Please retry after 30s.',
      status: 429,
      error: 'rate_limit_exceeded',
    },
  } as Record<string, unknown>,

  withThinkingMetadata: {
    thinkingMetadata: {
      budgetTokens: 10000,
      inputTokens: 3200,
      outputTokens: 1800,
      thinkingTokens: 5000,
      isRedacted: false,
    },
  } as Record<string, unknown>,

  withRetry: {
    retryInMs: 5000,
    retryAttempt: 2,
    maxRetries: 5,
  } as Record<string, unknown>,

  withHooks: {
    hookCount: 3,
    hookInfos: [
      { name: 'pre-commit', duration: 120 },
      { name: 'lint', duration: 450 },
    ],
    hookErrors: [],
  } as Record<string, unknown>,

  withHookErrors: {
    hookCount: 2,
    hookInfos: [{ name: 'typecheck', duration: 3200 }],
    hookErrors: [{ name: 'test', error: 'FAIL: 3 tests failed' }],
  } as Record<string, unknown>,

  full: {
    parentUuid: 'abc12345-dead-beef-cafe-123456789012',
    logicalParentUuid: 'def98765-fade-bead-cafe-987654321098',
    isSidechain: false,
    agentId: 'agent_main',
    uuid: '11111111-2222-3333-4444-555555555555',
    messageId: 'msg_001',
    sessionId: 'session_001',
    stopReason: 'end_turn',
    preventedContinuation: false,
    hasOutput: true,
    apiError: {
      message: 'Rate limit exceeded. Please retry after 30s.',
      status: 429,
      error: 'rate_limit_exceeded',
    },
    thinkingMetadata: {
      budgetTokens: 10000,
      inputTokens: 3200,
      outputTokens: 1800,
      thinkingTokens: 5000,
      isRedacted: false,
    },
    retryInMs: 5000,
    retryAttempt: 2,
    maxRetries: 5,
    hookCount: 3,
    hookInfos: [
      { name: 'pre-commit', duration: 120 },
      { name: 'lint', duration: 450 },
    ],
    hookErrors: [],
    permissionMode: 'auto-edit',
    durationMs: 12450,
    planContent: '1. Read file\n2. Edit middleware\n3. Run tests',
    prUrl: 'https://github.com/example/repo/pull/42',
    prNumber: 42,
    prRepository: 'example/repo',
    customTitle: 'Refactor auth middleware',
    promptId: 'prompt_abc123',
  } as Record<string, unknown>,

  empty: {} as Record<string, unknown>,
}

// ── Developer-mode UserBlock with rawJson ───────────────────────────────────

export const devUserBlocks = {
  withRawJson: {
    ...userBlocks.sent,
    id: 'ub_dev_001',
    rawJson: rawJsonFixtures.withLineage,
  } satisfies UserBlock,

  withImagePastes: {
    ...userBlocks.normal,
    id: 'ub_dev_002',
    rawJson: {
      ...rawJsonFixtures.withLineage,
      imagePasteIds: ['img_001', 'img_002'],
    },
  } satisfies UserBlock,
}

// ── Developer-mode AssistantBlock with rawJson ──────────────────────────────

export const devAssistantBlocks = {
  withRawJson: {
    ...assistantBlocks.textOnly,
    id: 'ab_dev_001',
    rawJson: rawJsonFixtures.full,
  } satisfies AssistantBlock,

  withPermissionMode: {
    ...assistantBlocks.withTools,
    id: 'ab_dev_002',
    rawJson: {
      permissionMode: 'auto-edit',
      durationMs: 8500,
      ...rawJsonFixtures.withStopReason,
    },
  } satisfies AssistantBlock,
}

// ── System Block Fixtures (variants not in base fixtures) ───────────────────

export const devSystemBlocks = {
  sessionInit: {
    type: 'system',
    id: 'dsb_001',
    variant: 'session_init',
    data: {
      type: 'session_init',
      sessionId: 'sess_abc123',
      tools: ['Bash', 'Read', 'Edit', 'Write', 'Grep', 'Glob'],
      model: 'claude-opus-4-6',
      mcpServers: [
        { name: 'postgres', status: 'connected' },
        { name: 'context7', status: 'connected' },
      ],
      permissionMode: 'auto-edit',
      slashCommands: ['/help', '/clear', '/commit'],
      claudeCodeVersion: '1.0.30',
      cwd: '/Users/dev/project',
      agents: ['research', 'codegen'],
      skills: ['commit', 'review-pr'],
      outputStyle: 'concise',
      capabilities: ['computer_use', 'code_execution'],
    } satisfies SessionInit,
  } satisfies SystemBlock,

  sessionStatus: {
    type: 'system',
    id: 'dsb_002',
    variant: 'session_status',
    data: {
      type: 'session_status',
      status: 'compacting',
      permissionMode: 'auto-edit',
    } satisfies SessionStatus,
  } satisfies SystemBlock,

  sessionStatusIdle: {
    type: 'system',
    id: 'dsb_002b',
    variant: 'session_status',
    data: {
      type: 'session_status',
      status: null,
      permissionMode: 'default',
    } satisfies SessionStatus,
  } satisfies SystemBlock,

  elicitationComplete: {
    type: 'system',
    id: 'dsb_003',
    variant: 'elicitation_complete',
    data: {
      type: 'elicitation_complete',
      mcpServerName: 'postgres',
      elicitationId: 'elic_001',
    } satisfies ElicitationComplete,
  } satisfies SystemBlock,

  hookEvent: {
    type: 'system',
    id: 'dsb_004',
    variant: 'hook_event',
    data: {
      type: 'hook_event',
      phase: 'response',
      hookId: 'hook_001',
      hookName: 'pre-commit',
      hookEventName: 'on_commit',
      stdout: 'All checks passed.\nLint: 0 errors, 0 warnings.',
      outcome: 'success',
    } satisfies HookEvent,
  } satisfies SystemBlock,

  hookEventError: {
    type: 'system',
    id: 'dsb_004b',
    variant: 'hook_event',
    data: {
      type: 'hook_event',
      phase: 'response',
      hookId: 'hook_002',
      hookName: 'typecheck',
      hookEventName: 'on_save',
      stderr: 'error TS2322: Type "string" is not assignable to type "number".',
      exitCode: 1,
      outcome: 'error',
    } satisfies HookEvent,
  } satisfies SystemBlock,

  filesSaved: {
    type: 'system',
    id: 'dsb_005',
    variant: 'files_saved',
    data: {
      type: 'files_saved',
      files: [
        { filename: 'src/auth/middleware.rs', fileId: 'f_001' },
        { filename: 'src/auth/validator.rs', fileId: 'f_002' },
      ],
      failed: [],
      processedAt: new Date().toISOString(),
    } satisfies FilesSaved,
  } satisfies SystemBlock,

  filesSavedWithFailures: {
    type: 'system',
    id: 'dsb_005b',
    variant: 'files_saved',
    data: {
      type: 'files_saved',
      files: [{ filename: 'src/lib.rs', fileId: 'f_003' }],
      failed: [{ filename: 'src/main.rs', error: 'Permission denied' }],
      processedAt: new Date().toISOString(),
    } satisfies FilesSaved,
  } satisfies SystemBlock,

  commandOutput: {
    type: 'system',
    id: 'dsb_006',
    variant: 'command_output',
    data: {
      type: 'command_output',
      content:
        '$ cargo test --workspace\n   Compiling auth v0.1.0\n   Compiling server v0.1.0\n     Running unittests src/lib.rs\nrunning 12 tests\ntest auth::validate_token ... ok\ntest auth::refresh_token ... ok\n\ntest result: ok. 12 passed; 0 failed; 0 ignored',
    } satisfies CommandOutput,
  } satisfies SystemBlock,

  streamDelta: {
    type: 'system',
    id: 'dsb_007',
    variant: 'stream_delta',
    data: {
      type: 'stream_delta',
      event: null,
      messageId: 'msg_delta_001',
      deltaType: 'content_block_delta',
      textDelta: 'Here is the refactored code...',
    } satisfies StreamDelta,
  } satisfies SystemBlock,

  unknown: {
    type: 'system',
    id: 'dsb_008',
    variant: 'unknown',
    data: {
      type: 'unknown_sdk_event',
      sdkType: 'experimental_feature_x',
      raw: { someField: 'someValue', count: 42 },
    } satisfies UnknownSdkEvent,
  } satisfies SystemBlock,

  localCommand: {
    type: 'system',
    id: 'dsb_009',
    variant: 'local_command',
    data: {
      type: 'system',
      subtype: 'local_command',
      content: '/clear',
    } satisfies LocalCommand,
  } satisfies SystemBlock,

  queueOperation: {
    type: 'system',
    id: 'dsb_010',
    variant: 'queue_operation',
    data: {
      type: 'queue-operation',
      operation: 'enqueue',
      timestamp: new Date().toISOString(),
      content: 'Fix the login bug',
    } satisfies QueueOperation,
  } satisfies SystemBlock,

  fileHistorySnapshot: {
    type: 'system',
    id: 'dsb_011',
    variant: 'file_history_snapshot',
    data: {
      type: 'file-history-snapshot',
      messageId: 'msg_snapshot_001',
      snapshot: {
        trackedFileBackups: {
          'src/auth/middleware.rs': {
            backupFileName: 'abc@v1',
            version: 1,
            backupTime: new Date().toISOString(),
          },
          'src/auth/validator.rs': {
            backupFileName: 'def@v1',
            version: 1,
            backupTime: new Date().toISOString(),
          },
          'src/auth/session.rs': {
            backupFileName: 'ghi@v1',
            version: 1,
            backupTime: new Date().toISOString(),
          },
        },
        timestamp: new Date().toISOString(),
      },
      isSnapshotUpdate: false,
      fileCount: 3,
      files: ['src/auth/middleware.rs', 'src/auth/validator.rs', 'src/auth/session.rs'],
      isIncremental: true,
    } satisfies FileHistorySnapshot,
  } satisfies SystemBlock,

  aiTitle: {
    type: 'system',
    id: 'dsb_012',
    variant: 'ai_title',
    data: {
      type: 'ai-title',
      sessionId: 'sess_abc123',
      aiTitle: 'Refactor authentication middleware',
    } satisfies AiTitle,
  } satisfies SystemBlock,

  lastPrompt: {
    type: 'system',
    id: 'dsb_013',
    variant: 'last_prompt',
    data: {
      type: 'last-prompt',
      sessionId: 'sess_abc123',
      lastPrompt: 'Can you run the tests one more time?',
    } satisfies LastPrompt,
  } satisfies SystemBlock,

  informational: {
    type: 'system',
    id: 'dsb_014',
    variant: 'informational',
    data: {
      content: 'Session resumed after network reconnection',
    } satisfies Informational,
  } satisfies SystemBlock,
}

// ── System blocks with rawJson for detail panels ────────────────────────────

export const devSystemBlocksWithRawJson = {
  withRetry: {
    ...devSystemBlocks.sessionStatus,
    id: 'dsb_raw_001',
    rawJson: rawJsonFixtures.withRetry,
  } satisfies SystemBlock,

  withApiError: {
    ...devSystemBlocks.sessionStatus,
    id: 'dsb_raw_002',
    rawJson: rawJsonFixtures.withApiError,
  } satisfies SystemBlock,

  withHooks: {
    ...devSystemBlocks.hookEvent,
    id: 'dsb_raw_003',
    rawJson: rawJsonFixtures.withHooks,
  } satisfies SystemBlock,

  withAll: {
    ...devSystemBlocks.sessionInit,
    id: 'dsb_raw_004',
    rawJson: rawJsonFixtures.full,
  } satisfies SystemBlock,
}

// ── TurnBoundary with permission denials ────────────────────────────────────

export const devTurnBoundaryBlocks = {
  withPermissionDenials: {
    type: 'turn_boundary' as const,
    id: 'dtb_001',
    success: true,
    totalCostUsd: 0.045,
    numTurns: 3,
    durationMs: 25000,
    usage: { input_tokens: 8000, output_tokens: 3500, cache_read_input_tokens: 5000 },
    modelUsage: {
      'claude-opus-4-6': {
        inputTokens: 8000,
        outputTokens: 3500,
        cacheReadInputTokens: 5000,
        cacheCreationInputTokens: 0,
        webSearchRequests: 0,
        costUSD: 0.045,
        contextWindow: 200000,
        maxOutputTokens: 16384,
      },
    },
    permissionDenials: [
      {
        toolName: 'Bash',
        toolUseId: 'tu_denied_001',
        toolInput: { command: 'rm -rf /' },
      },
      {
        toolName: 'Write',
        toolUseId: 'tu_denied_002',
        toolInput: { file_path: '/etc/passwd', content: 'hacked' },
      },
    ],
    stopReason: 'end_turn',
  },
}
