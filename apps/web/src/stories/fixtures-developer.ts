/**
 * Additional fixture data for developer mode Storybook stories.
 * rawJson fixtures for detail panels + system block variant fixtures not in base fixtures.
 */
import type { AssistantBlock, SystemBlock, UserBlock } from '@claude-view/shared/types/blocks'
import type {
  AgentName,
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

// ── rawJson fixtures sourced from real JSONL session data ─────────────────────
// Evidence: evidence-audit scanned 5,932 files / ~998K lines.
// Shapes below match real `stop_hook_summary`, `api_error`, `turn_duration`
// envelope fields observed in production session JSONL.

export const rawJsonFixtures = {
  /** Real message lineage fields from a stop_hook_summary event — UUIDs anonymized */
  withLineage: {
    parentUuid: 'a1b2c3d4-5678-4abc-9def-111111111111',
    isSidechain: false,
    uuid: 'a1b2c3d4-5678-4abc-9def-222222222222',
    sessionId: 'a1b2c3d4-5678-4abc-9def-000000000001',
    toolUseID: 'a1b2c3d4-5678-4abc-9def-333333333333',
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

  /** Real api_error shape — ECONNRESET against api.anthropic.com */
  withApiError: {
    apiError: {
      cause: {
        code: 'ECONNRESET',
        path: 'https://api.anthropic.com/v1/messages?beta=true',
        errno: 0,
      },
      error: {
        cause: {
          code: 'ECONNRESET',
          path: 'https://api.anthropic.com/v1/messages?beta=true',
          errno: 0,
        },
      },
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

  /** Real retry envelope from api_error event — attempt 4 of 10 */
  withRetry: {
    retryInMs: 4809.996137160989,
    retryAttempt: 4,
    maxRetries: 10,
  } as Record<string, unknown>,

  /** Real stop_hook_summary shape — Live Monitor + token counter + plugin hook */
  withHooks: {
    hookCount: 3,
    hookInfos: [
      { command: 'Live Monitor' },
      { command: '~/.claude/count_tokens.js', durationMs: 3 },
      { command: '${CLAUDE_PLUGIN_ROOT}/hooks/stop-hook.sh', durationMs: 1 },
    ],
    hookErrors: [],
    preventedContinuation: false,
    stopReason: '',
    hasOutput: true,
  } as Record<string, unknown>,

  /** Real stop_hook_summary with ENOENT hook errors — real error shape from production */
  withHookErrors: {
    hookCount: 3,
    hookInfos: [
      { command: 'Live Monitor' },
      { command: '~/.claude/count_tokens.js', durationMs: 3 },
      { command: '${CLAUDE_PLUGIN_ROOT}/hooks/stop-hook.sh', durationMs: 1 },
    ],
    hookErrors: [
      "Failed with non-blocking status code: Error occurred while executing hook command: ENOENT: no such file or directory, posix_spawn '/bin/sh'",
      "Failed with non-blocking status code: Error occurred while executing hook command: ENOENT: no such file or directory, posix_spawn '/bin/sh'",
    ],
    preventedContinuation: false,
    stopReason: '',
    hasOutput: true,
  } as Record<string, unknown>,

  /** Composite: real shapes from production — lineage + hooks + api_error + retry. UUIDs anonymized. */
  full: {
    parentUuid: 'a1b2c3d4-5678-4abc-9def-111111111111',
    isSidechain: false,
    uuid: 'a1b2c3d4-5678-4abc-9def-222222222222',
    sessionId: 'a1b2c3d4-5678-4abc-9def-000000000001',
    toolUseID: 'a1b2c3d4-5678-4abc-9def-333333333333',
    stopReason: 'end_turn',
    preventedContinuation: false,
    hasOutput: true,
    apiError: {
      cause: {
        code: 'ECONNRESET',
        path: 'https://api.anthropic.com/v1/messages?beta=true',
        errno: 0,
      },
    },
    retryInMs: 4809.996137160989,
    retryAttempt: 4,
    maxRetries: 10,
    hookCount: 3,
    hookInfos: [
      { command: 'Live Monitor' },
      { command: '~/.claude/count_tokens.js', durationMs: 128 },
      { command: '${CLAUDE_PLUGIN_ROOT}/hooks/stop-hook.sh', durationMs: 54 },
    ],
    hookErrors: [],
    permissionMode: 'auto-edit',
    durationMs: 91982,
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

  /** Real local_command shape — /config slash command from production JSONL */
  localCommand: {
    type: 'system',
    id: 'dsb_009',
    variant: 'local_command',
    data: {
      type: 'system',
      subtype: 'local_command',
      content:
        '<command-name>/config</command-name>\n            <command-message>config</command-message>\n            <command-args></command-args>',
    } satisfies LocalCommand,
  } satisfies SystemBlock,

  /** Real queue-operation shape — user enqueued message while agent was busy */
  queueOperation: {
    type: 'system',
    id: 'dsb_010',
    variant: 'queue_operation',
    data: {
      type: 'queue-operation',
      operation: 'enqueue',
      timestamp: '2026-03-07T08:59:44.564Z',
      content: 'so this worktree I guess we can clean it ?',
    } satisfies QueueOperation,
  } satisfies SystemBlock,

  /** Real file-history-snapshot shape with tracked backup — UUIDs anonymized */
  fileHistorySnapshot: {
    type: 'system',
    id: 'dsb_011',
    variant: 'file_history_snapshot',
    data: {
      type: 'file-history-snapshot',
      messageId: 'a1b2c3d4-5678-4abc-9def-444444444444',
      snapshot: {
        messageId: 'a1b2c3d4-5678-4abc-9def-555555555555',
        trackedFileBackups: {
          'src/components/Dashboard.tsx': {
            backupFileName: '',
            version: 1,
            backupTime: '2026-02-26T17:42:18.026Z',
          },
        },
        timestamp: '2026-02-26T17:25:23.698Z',
      },
      isSnapshotUpdate: true,
      fileCount: 1,
      files: ['src/components/Dashboard.tsx'],
      isIncremental: false,
    } satisfies FileHistorySnapshot,
  } satisfies SystemBlock,

  /** Real ai-title shape — verified by evidence-audit baseline */
  aiTitle: {
    type: 'system',
    id: 'dsb_012',
    variant: 'ai_title',
    data: {
      type: 'ai-title',
      sessionId: 'sess-100',
      aiTitle: 'System Only Session',
    } satisfies AiTitle,
  } satisfies SystemBlock,

  /** Real last-prompt shape — verified by evidence-audit baseline */
  lastPrompt: {
    type: 'system',
    id: 'dsb_013',
    variant: 'last_prompt',
    data: {
      type: 'last-prompt',
      sessionId: 'sess-100',
      lastPrompt: 'hello world',
    } satisfies LastPrompt,
  } satisfies SystemBlock,

  /** informational subtype — only 2 occurrences across ~998K lines (evidence-audit baseline) */
  informational: {
    type: 'system',
    id: 'dsb_014',
    variant: 'informational',
    data: {
      content: 'Conversation compacted',
      message: 'Conversation compacted',
    } satisfies Informational,
  } satisfies SystemBlock,

  /** agent-name — emitted when session runs as a subagent */
  agentName: {
    type: 'system',
    id: 'dsb_015',
    variant: 'agent_name',
    data: {
      type: 'agent-name',
      agentName: 'code-reviewer',
      sessionId: 'sess-100',
    } satisfies AgentName,
  } satisfies SystemBlock,
}

// ── System blocks with rawJson for detail panels ────────────────────────────

// ── System blocks with rawJson — real envelope fields from production JSONL ──

export const devSystemBlocksWithRawJson = {
  /** Real api_error retry envelope — attempt 4/10, 4.8s backoff */
  withRetry: {
    ...devSystemBlocks.sessionStatus,
    id: 'dsb_raw_001',
    rawJson: rawJsonFixtures.withRetry,
  } satisfies SystemBlock,

  /** Real api_error — ECONNRESET against api.anthropic.com */
  withApiError: {
    ...devSystemBlocks.sessionStatus,
    id: 'dsb_raw_002',
    rawJson: rawJsonFixtures.withApiError,
  } satisfies SystemBlock,

  /** Real stop_hook_summary — Live Monitor + token counter + plugin hook */
  withHooks: {
    ...devSystemBlocks.hookEvent,
    id: 'dsb_raw_003',
    rawJson: rawJsonFixtures.withHooks,
  } satisfies SystemBlock,

  /** Real stop_hook_summary with ENOENT hook errors */
  withHookErrors: {
    ...devSystemBlocks.hookEvent,
    id: 'dsb_raw_003b',
    rawJson: rawJsonFixtures.withHookErrors,
  } satisfies SystemBlock,

  /** Composite of all real rawJson fields — lineage + hooks + api_error + retry + 91.9s duration */
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
