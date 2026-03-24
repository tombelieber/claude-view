/**
 * Shared fixture data for Storybook stories.
 * All block types and protocol event fixtures in one place.
 */
import type {
  AssistantBlock,
  ConversationBlock,
  InteractionBlock,
  NoticeBlock,
  ProgressBlock,
  SystemBlock,
  ToolExecution,
  TurnBoundaryBlock,
  UserBlock,
} from '@claude-view/shared/types/blocks'
import type {
  AskQuestion,
  AssistantError,
  AuthStatus,
  ContextCompacted,
  Elicitation,
  ErrorEvent,
  PermissionRequest,
  PlanApproval,
  PromptSuggestion,
  RateLimit,
  SessionClosed,
  QueueOperation,
  TaskNotification,
  TaskProgressEvent,
  TaskStarted,
} from '@claude-view/shared/types/sidecar-protocol'

// ── Timestamps ──────────────────────────────────────────────────────────────

const NOW = Math.floor(Date.now() / 1000)
const FIVE_MIN_AGO = NOW - 300
const TEN_MIN_AGO = NOW - 600

// ── Tool Executions ─────────────────────────────────────────────────────────

export const toolExecutions = {
  bashRunning: {
    toolName: 'Bash',
    toolInput: { command: 'cargo test --workspace' },
    toolUseId: 'tu_bash_001',
    status: 'running',
    category: 'builtin',
    progress: { elapsedSeconds: 3.2 },
  } satisfies ToolExecution,

  bashComplete: {
    toolName: 'Bash',
    toolInput: { command: 'ls -la src/' },
    toolUseId: 'tu_bash_002',
    status: 'complete',
    category: 'builtin',
    duration: 1240,
    result: {
      output:
        'total 48\ndrwxr-xr-x  12 user staff  384 Mar 21 10:00 .\n-rw-r--r--   1 user staff 1234 Mar 21 09:55 main.rs\n-rw-r--r--   1 user staff  890 Mar 21 09:50 lib.rs',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,

  bashError: {
    toolName: 'Bash',
    toolInput: { command: 'rm -rf /nonexistent' },
    toolUseId: 'tu_bash_003',
    status: 'error',
    category: 'builtin',
    duration: 85,
    result: {
      output: 'rm: /nonexistent: No such file or directory',
      isError: true,
      isReplay: false,
    },
  } satisfies ToolExecution,

  readComplete: {
    toolName: 'Read',
    toolInput: { file_path: '/Users/dev/project/src/main.rs' },
    toolUseId: 'tu_read_001',
    status: 'complete',
    category: 'builtin',
    duration: 42,
    result: {
      output: 'fn main() {\n    println!("Hello, world!");\n}',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,

  editComplete: {
    toolName: 'Edit',
    toolInput: {
      file_path: '/Users/dev/project/src/lib.rs',
      old_string: 'fn old_function()',
      new_string: 'fn new_function()',
    },
    toolUseId: 'tu_edit_001',
    status: 'complete',
    category: 'builtin',
    duration: 310,
    result: {
      output: 'Successfully edited file',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,

  grepRunning: {
    toolName: 'Grep',
    toolInput: { pattern: 'TODO|FIXME|HACK', path: 'src/' },
    toolUseId: 'tu_grep_001',
    status: 'running',
    category: 'builtin',
    progress: { elapsedSeconds: 1.5 },
  } satisfies ToolExecution,

  /** Edit with diff-like output — ContentRenderer should color +/- lines */
  editWithDiff: {
    toolName: 'Edit',
    toolInput: {
      file_path: 'src/auth/middleware.rs',
      old_string: 'fn validate(&self)',
      new_string: 'fn validate(&mut self)',
    },
    toolUseId: 'tu_edit_diff_001',
    status: 'complete',
    category: 'builtin',
    duration: 150,
    result: {
      output:
        'diff --git a/src/auth/middleware.rs b/src/auth/middleware.rs\n--- a/src/auth/middleware.rs\n+++ b/src/auth/middleware.rs\n@@ -42,7 +42,7 @@\n-    fn validate(&self) -> Result<Token> {\n+    fn validate(&mut self) -> Result<Token> {\n         self.cache.check()?;',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,

  /** MCP tool — showcases category badge for non-builtin */
  mcpQuery: {
    toolName: 'mcp__postgres__query',
    toolInput: {
      query: "SELECT count(*) FROM sessions WHERE created_at > now() - interval '1 day'",
    },
    toolUseId: 'tu_mcp_001',
    status: 'complete',
    category: 'mcp',
    duration: 890,
    result: {
      output: '{"count": 1463}',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,

  /** Agent/Task tool — showcases agent category */
  agentTask: {
    toolName: 'Task',
    toolInput: {
      description: 'Research auth patterns',
      prompt: 'Find all auth middleware implementations...',
    },
    toolUseId: 'tu_agent_001',
    status: 'complete',
    category: 'agent',
    duration: 45200,
    summary: 'Found 3 middleware patterns across the codebase',
    result: {
      output:
        'Analyzed 12 files, identified 3 distinct auth patterns:\n1. JWT bearer token validation\n2. Session cookie authentication\n3. API key header check',
      isError: false,
      isReplay: false,
    },
  } satisfies ToolExecution,
}

// ── User Blocks ─────────────────────────────────────────────────────────────

export const userBlocks = {
  normal: {
    type: 'user',
    id: 'ub_001',
    text: 'Can you help me refactor the authentication middleware?',
    timestamp: TEN_MIN_AGO,
    status: 'sent',
  } satisfies UserBlock,

  optimistic: {
    type: 'user',
    id: 'ub_002',
    text: 'Let me check the test results...',
    timestamp: NOW,
    status: 'optimistic',
    localId: 'local_001',
  } satisfies UserBlock,

  sending: {
    type: 'user',
    id: 'ub_003',
    text: 'Run cargo test --workspace please',
    timestamp: NOW,
    status: 'sending',
    localId: 'local_002',
  } satisfies UserBlock,

  sent: {
    type: 'user',
    id: 'ub_004',
    text: 'Looks good, ship it!',
    timestamp: FIVE_MIN_AGO,
    status: 'sent',
  } satisfies UserBlock,

  failed: {
    type: 'user',
    id: 'ub_005',
    text: 'This message failed to send because of a connection issue.',
    timestamp: NOW,
    status: 'failed',
    localId: 'local_003',
  } satisfies UserBlock,

  withImage: {
    type: 'user',
    id: 'ub_007',
    text: 'Here is a screenshot of the bug:',
    timestamp: FIVE_MIN_AGO,
    status: 'sent',
    images: [
      {
        sourceType: 'base64',
        mediaType: 'image/png',
        data: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
      },
    ],
  } satisfies UserBlock,

  sidechain: {
    type: 'user',
    id: 'ub_008',
    text: 'This message is from a sidechain branch.',
    timestamp: FIVE_MIN_AGO,
    status: 'sent',
    isSidechain: true,
  } satisfies UserBlock,

  fromAgent: {
    type: 'user',
    id: 'ub_009',
    text: 'Running agent task in background...',
    timestamp: FIVE_MIN_AGO,
    status: 'sent',
    agentId: 'a7f2e9b',
  } satisfies UserBlock,

  long: {
    type: 'user',
    id: 'ub_006',
    text: 'I need you to do several things:\n\n1. First, read the current implementation of the WebSocket handler\n2. Then refactor it to use the new event-driven pattern\n3. Make sure all the tests still pass\n4. Update the documentation\n\nThe key requirement is that we maintain backwards compatibility with the existing protocol while adding the new features. The streaming delta events should be processed in order and any out-of-sequence events should be buffered until the gap is filled.\n\nAlso, please check if there are any memory leaks in the current implementation — I noticed the RSS growing over time during long sessions.',
    timestamp: TEN_MIN_AGO,
    status: 'sent',
  } satisfies UserBlock,
}

// ── Assistant Blocks ────────────────────────────────────────────────────────

export const assistantBlocks = {
  textOnly: {
    type: 'assistant',
    id: 'ab_001',
    segments: [
      {
        kind: 'text',
        text: "I'll help you refactor the authentication middleware. Let me start by reading the current implementation to understand the structure.",
      },
    ],
    streaming: false,
    timestamp: TEN_MIN_AGO + 5,
  } satisfies AssistantBlock,

  streaming: {
    type: 'assistant',
    id: 'ab_002',
    segments: [
      {
        kind: 'text',
        text: 'Looking at the codebase, I can see the middleware is currently using a session-based approach. Let me',
      },
    ],
    streaming: true,
    timestamp: NOW,
  } satisfies AssistantBlock,

  withThinking: {
    type: 'assistant',
    id: 'ab_003',
    segments: [
      {
        kind: 'text',
        text: "Based on my analysis, the best approach is to extract the token validation into a separate service. Here's the plan:\n\n1. Create a `TokenValidator` struct\n2. Move the JWT verification logic there\n3. Add caching for validated tokens\n4. Update the middleware to use the new service",
      },
    ],
    thinking:
      "The user wants to refactor auth middleware. Let me think about the best approach...\n\nThe current code has tight coupling between token validation and session management. The middleware does too many things:\n- JWT verification\n- Session lookup\n- Permission checking\n- Rate limiting\n\nI should suggest extracting each concern into its own module. The token validator is the most impactful extraction because it's used in multiple places.",
    streaming: false,
    timestamp: TEN_MIN_AGO + 10,
  } satisfies AssistantBlock,

  withTools: {
    type: 'assistant',
    id: 'ab_004',
    segments: [
      { kind: 'text', text: 'Let me read the current middleware implementation first.' },
      { kind: 'tool', execution: toolExecutions.readComplete },
      {
        kind: 'text',
        text: 'I can see the structure. Now let me make the changes:',
      },
      { kind: 'tool', execution: toolExecutions.editComplete },
      {
        kind: 'text',
        text: 'Done! The middleware has been refactored. Let me verify the tests pass:',
      },
      { kind: 'tool', execution: toolExecutions.bashComplete },
    ],
    streaming: false,
    timestamp: FIVE_MIN_AGO,
  } satisfies AssistantBlock,

  withRunningTool: {
    type: 'assistant',
    id: 'ab_005',
    segments: [
      { kind: 'text', text: 'Running the test suite now to verify everything works:' },
      { kind: 'tool', execution: toolExecutions.bashRunning },
    ],
    streaming: true,
    timestamp: NOW,
  } satisfies AssistantBlock,

  withToolError: {
    type: 'assistant',
    id: 'ab_006',
    segments: [
      { kind: 'text', text: 'Let me try to clean up the temporary files:' },
      { kind: 'tool', execution: toolExecutions.bashError },
      {
        kind: 'text',
        text: "The file doesn't exist — that's actually fine, it means the cleanup was already done.",
      },
    ],
    streaming: false,
    timestamp: FIVE_MIN_AGO + 30,
  } satisfies AssistantBlock,

  markdown: {
    type: 'assistant',
    id: 'ab_007',
    segments: [
      {
        kind: 'text',
        text: `Here's a summary of the changes:

## Refactored Modules

| Module | Before | After |
|--------|--------|-------|
| \`auth/middleware.rs\` | 450 lines | 120 lines |
| \`auth/validator.rs\` | — | 180 lines |
| \`auth/session.rs\` | — | 90 lines |

### Key improvements

- **Separation of concerns**: Each module handles exactly one responsibility
- **Testability**: \`TokenValidator\` can be unit tested with mock keys
- **Performance**: Added LRU cache for validated tokens

\`\`\`rust
pub struct TokenValidator {
    jwks: JwkSet,
    cache: LruCache<String, Claims>,
}

impl TokenValidator {
    pub fn validate(&mut self, token: &str) -> Result<Claims> {
        if let Some(claims) = self.cache.get(token) {
            return Ok(claims.clone());
        }
        let claims = self.verify_jwt(token)?;
        self.cache.put(token.to_string(), claims.clone());
        Ok(claims)
    }
}
\`\`\`

> **Note**: The cache TTL should match the JWT expiry to avoid serving stale claims.`,
      },
    ],
    streaming: false,
    timestamp: FIVE_MIN_AGO + 60,
  } satisfies AssistantBlock,

  sidechainReply: {
    type: 'assistant',
    id: 'ab_sidechain',
    segments: [{ kind: 'text', text: 'This is a response from a sidechain conversation branch.' }],
    streaming: false,
    timestamp: FIVE_MIN_AGO + 30,
    isSidechain: true,
  } satisfies AssistantBlock,

  fromAgent: {
    type: 'assistant',
    id: 'ab_agent',
    segments: [
      { kind: 'text', text: 'I found the relevant files. Here is my analysis from the sub-agent:' },
    ],
    streaming: false,
    timestamp: FIVE_MIN_AGO + 45,
    agentId: 'a7f2e9b',
  } satisfies AssistantBlock,

  /** Showcases ALL ToolCard variants: diff output, JSON output, MCP, agent, category badges, durations */
  withAllToolVariants: {
    type: 'assistant',
    id: 'ab_008',
    segments: [
      { kind: 'text', text: 'Here are all ToolCard variants — diff, JSON, MCP, and agent tools:' },
      { kind: 'tool', execution: toolExecutions.editWithDiff },
      { kind: 'tool', execution: toolExecutions.mcpQuery },
      { kind: 'tool', execution: toolExecutions.agentTask },
      { kind: 'tool', execution: toolExecutions.bashRunning },
      { kind: 'tool', execution: toolExecutions.grepRunning },
    ],
    streaming: false,
    timestamp: FIVE_MIN_AGO + 90,
  } satisfies AssistantBlock,
}

// ── Notice Blocks ───────────────────────────────────────────────────────────

export const noticeBlocks = {
  assistantError: {
    type: 'notice',
    id: 'nb_001',
    variant: 'assistant_error',
    data: {
      type: 'assistant_error',
      error: 'rate_limit',
      messageId: 'msg_001',
    } satisfies AssistantError,
  } satisfies NoticeBlock,

  billingError: {
    type: 'notice',
    id: 'nb_001b',
    variant: 'assistant_error',
    data: {
      type: 'assistant_error',
      error: 'billing_error',
      messageId: 'msg_002',
    } satisfies AssistantError,
  } satisfies NoticeBlock,

  authFailed: {
    type: 'notice',
    id: 'nb_001c',
    variant: 'assistant_error',
    data: {
      type: 'assistant_error',
      error: 'authentication_failed',
      messageId: 'msg_003',
    } satisfies AssistantError,
  } satisfies NoticeBlock,

  serverError: {
    type: 'notice',
    id: 'nb_001d',
    variant: 'assistant_error',
    data: {
      type: 'assistant_error',
      error: 'server_error',
      messageId: 'msg_004',
    } satisfies AssistantError,
  } satisfies NoticeBlock,

  rateLimitWarning: {
    type: 'notice',
    id: 'nb_002',
    variant: 'rate_limit',
    data: {
      type: 'rate_limit',
      status: 'allowed_warning',
      utilization: 0.85,
    } satisfies RateLimit,
  } satisfies NoticeBlock,

  rateLimitRejected: {
    type: 'notice',
    id: 'nb_003',
    variant: 'rate_limit',
    data: {
      type: 'rate_limit',
      status: 'rejected',
      resetsAt: NOW + 120,
    } satisfies RateLimit,
  } satisfies NoticeBlock,

  contextCompacted: {
    type: 'notice',
    id: 'nb_004',
    variant: 'context_compacted',
    data: {
      type: 'context_compacted',
      trigger: 'auto',
      preTokens: 145000,
    } satisfies ContextCompacted,
  } satisfies NoticeBlock,

  contextCompactedManual: {
    type: 'notice',
    id: 'nb_004b',
    variant: 'context_compacted',
    data: {
      type: 'context_compacted',
      trigger: 'manual',
      preTokens: 80000,
    } satisfies ContextCompacted,
  } satisfies NoticeBlock,

  authenticating: {
    type: 'notice',
    id: 'nb_005',
    variant: 'auth_status',
    data: {
      type: 'auth_status',
      isAuthenticating: true,
      output: [],
    } satisfies AuthStatus,
  } satisfies NoticeBlock,

  authError: {
    type: 'notice',
    id: 'nb_006',
    variant: 'auth_status',
    data: {
      type: 'auth_status',
      isAuthenticating: false,
      output: [],
      error: 'Invalid API key. Please check your credentials.',
    } satisfies AuthStatus,
  } satisfies NoticeBlock,

  sessionClosed: {
    type: 'notice',
    id: 'nb_007',
    variant: 'session_closed',
    data: { type: 'session_closed', reason: 'User disconnected' } satisfies SessionClosed,
  } satisfies NoticeBlock,

  error: {
    type: 'notice',
    id: 'nb_008',
    variant: 'error',
    data: {
      type: 'error',
      message: 'WebSocket connection lost. Attempting to reconnect...',
      fatal: false,
    } satisfies ErrorEvent,
  } satisfies NoticeBlock,

  fatalError: {
    type: 'notice',
    id: 'nb_009',
    variant: 'error',
    data: {
      type: 'error',
      message: 'Sidecar process terminated unexpectedly. Please restart the session.',
      fatal: true,
    } satisfies ErrorEvent,
  } satisfies NoticeBlock,

  promptSuggestion: {
    type: 'notice',
    id: 'nb_010',
    variant: 'prompt_suggestion',
    data: {
      type: 'prompt_suggestion',
      suggestion: 'Show me the test results',
    } satisfies PromptSuggestion,
  } satisfies NoticeBlock,

  sessionResumed: {
    type: 'notice',
    id: 'nb_011',
    variant: 'session_resumed',
    data: null,
  } satisfies NoticeBlock,

  rateLimitWithRetry: {
    type: 'notice',
    id: 'nb_012',
    variant: 'rate_limit',
    data: {
      type: 'rate_limit',
      status: 'rejected',
      utilization: 1.0,
    } satisfies RateLimit,
    retryInMs: 5000,
    retryAttempt: 2,
    maxRetries: 3,
  } satisfies NoticeBlock,
}

// ── Turn Boundary Blocks ────────────────────────────────────────────────────

export const turnBoundaryBlocks = {
  success: {
    type: 'turn_boundary',
    id: 'tb_001',
    success: true,
    totalCostUsd: 0.0342,
    numTurns: 1,
    durationMs: 12500,
    usage: { input_tokens: 4200, output_tokens: 1800, cache_read_input_tokens: 3100 },
    modelUsage: {
      'claude-opus-4-6': {
        inputTokens: 4200,
        outputTokens: 1800,
        cacheReadInputTokens: 3100,
        cacheCreationInputTokens: 0,
        webSearchRequests: 0,
        costUSD: 0.0342,
        contextWindow: 200000,
        maxOutputTokens: 16384,
      },
    },
    permissionDenials: [],
    stopReason: 'end_turn',
  } satisfies TurnBoundaryBlock,

  cheap: {
    type: 'turn_boundary',
    id: 'tb_002',
    success: true,
    totalCostUsd: 0.0008,
    numTurns: 1,
    durationMs: 2100,
    usage: { input_tokens: 500, output_tokens: 200 },
    modelUsage: {},
    permissionDenials: [],
    stopReason: 'end_turn',
  } satisfies TurnBoundaryBlock,

  free: {
    type: 'turn_boundary',
    id: 'tb_003',
    success: true,
    totalCostUsd: 0,
    numTurns: 1,
    durationMs: 500,
    usage: {},
    modelUsage: {},
    permissionDenials: [],
    stopReason: 'end_turn',
  } satisfies TurnBoundaryBlock,

  error: {
    type: 'turn_boundary',
    id: 'tb_004',
    success: false,
    totalCostUsd: 0.012,
    numTurns: 1,
    durationMs: 8000,
    usage: { input_tokens: 2000, output_tokens: 600 },
    modelUsage: {},
    permissionDenials: [],
    stopReason: null,
    error: {
      subtype: 'error_during_execution',
      messages: ['Tool execution failed: ENOENT /tmp/missing.txt'],
    },
  } satisfies TurnBoundaryBlock,

  withHookErrors: {
    type: 'turn_boundary',
    id: 'tb_006',
    success: false,
    totalCostUsd: 0.018,
    numTurns: 1,
    durationMs: 9500,
    usage: { input_tokens: 3000, output_tokens: 900 },
    modelUsage: {},
    permissionDenials: [],
    stopReason: 'end_turn',
    hookCount: 3,
    hookInfos: [
      { hookName: 'pre-commit-lint', hookEvent: 'PreToolUse', status: 'passed' },
      { hookName: 'security-scan', hookEvent: 'PreToolUse', status: 'failed' },
      { hookName: 'post-deploy-check', hookEvent: 'PostToolUse', status: 'passed' },
    ],
    hookErrors: ['security-scan: found hardcoded API key in config.ts:42'],
  } satisfies TurnBoundaryBlock,

  preventedContinuation: {
    type: 'turn_boundary',
    id: 'tb_007',
    success: false,
    totalCostUsd: 0.005,
    numTurns: 1,
    durationMs: 3000,
    usage: { input_tokens: 1000, output_tokens: 200 },
    modelUsage: {},
    permissionDenials: [],
    stopReason: 'end_turn',
    preventedContinuation: true,
    hookCount: 1,
    hookInfos: [{ hookName: 'guard-mode', hookEvent: 'PreToolUse', status: 'blocked' }],
    hookErrors: ['guard-mode: destructive command blocked (rm -rf)'],
  } satisfies TurnBoundaryBlock,

  maxTurns: {
    type: 'turn_boundary',
    id: 'tb_005',
    success: false,
    totalCostUsd: 0.285,
    numTurns: 25,
    durationMs: 180000,
    usage: { input_tokens: 125000, output_tokens: 45000 },
    modelUsage: {},
    permissionDenials: [],
    stopReason: null,
    error: {
      subtype: 'error_max_turns',
      messages: ['Reached maximum number of turns (25)'],
    },
  } satisfies TurnBoundaryBlock,
}

// ── System Blocks ───────────────────────────────────────────────────────────

export const systemBlocks = {
  taskStarted: {
    type: 'system',
    id: 'sb_001',
    variant: 'task_started',
    data: {
      type: 'task_started',
      taskId: 'task_001',
      description: 'Researching authentication best practices',
      taskType: 'agent',
    } satisfies TaskStarted,
  } satisfies SystemBlock,

  taskProgress: {
    type: 'system',
    id: 'sb_002',
    variant: 'task_progress',
    data: {
      type: 'task_progress',
      taskId: 'task_001',
      description: 'Researching authentication best practices',
      lastToolName: 'WebSearch',
      summary: 'Found 3 relevant articles on JWT middleware patterns',
      usage: { totalTokens: 8500, toolUses: 4, durationMs: 15000 },
    } satisfies TaskProgressEvent,
  } satisfies SystemBlock,

  taskCompleted: {
    type: 'system',
    id: 'sb_003',
    variant: 'task_notification',
    data: {
      type: 'task_notification',
      taskId: 'task_001',
      status: 'completed',
      outputFile: '/tmp/task_001_output.md',
      summary: 'Research complete — found 5 patterns for JWT middleware in Axum',
      usage: { totalTokens: 12000, toolUses: 8, durationMs: 25000 },
    } satisfies TaskNotification,
  } satisfies SystemBlock,

  taskFailed: {
    type: 'system',
    id: 'sb_004',
    variant: 'task_notification',
    data: {
      type: 'task_notification',
      taskId: 'task_002',
      status: 'failed',
      outputFile: '/tmp/task_002_output.md',
      summary: 'Failed to connect to MCP server: timeout after 30s',
    } satisfies TaskNotification,
  } satisfies SystemBlock,

  queueOperation: {
    type: 'system',
    id: 'sb_005',
    variant: 'queue_operation',
    data: {
      type: 'queue-operation',
      operation: 'enqueue',
      timestamp: new Date().toISOString(),
      content: 'Fix the login bug on the settings page',
    } satisfies QueueOperation,
  } satisfies SystemBlock,

  prLink: {
    type: 'system',
    id: 'sb_006',
    variant: 'pr_link',
    data: {
      type: 'pr-link',
      prNumber: 142,
      prUrl: 'https://github.com/tombelieber/claude-view/pull/142',
      prRepository: 'tombelieber/claude-view',
    },
  } as SystemBlock,

  customTitle: {
    type: 'system',
    id: 'sb_007',
    variant: 'custom_title',
    data: {
      type: 'custom-title',
      customTitle: 'Refactor auth middleware for compliance',
    },
  } as SystemBlock,

  planContent: {
    type: 'system',
    id: 'sb_008',
    variant: 'plan_content',
    data: {
      type: 'system',
      planContent:
        '## Implementation Plan\n\n1. Extract middleware to separate module\n2. Add JWT validation\n3. Update route guards\n4. Add integration tests',
    },
  } as SystemBlock,
}

// ── Interaction / Protocol Data ─────────────────────────────────────────────

export const permissionRequests = {
  bash: {
    type: 'permission_request',
    requestId: 'perm_001',
    toolName: 'Bash',
    toolInput: { command: 'cargo test --workspace -- --nocapture' },
    toolUseID: 'tu_perm_001',
    timeoutMs: 30000,
  } satisfies PermissionRequest,

  edit: {
    type: 'permission_request',
    requestId: 'perm_002',
    toolName: 'Edit',
    toolInput: {
      file_path: '/Users/dev/project/src/auth/middleware.rs',
      old_string: 'fn validate_token(&self)',
      new_string: 'fn validate_token(&mut self)',
    },
    toolUseID: 'tu_perm_002',
    timeoutMs: 60000,
    decisionReason: 'File is outside the project directory',
  } satisfies PermissionRequest,

  write: {
    type: 'permission_request',
    requestId: 'perm_003',
    toolName: 'Write',
    toolInput: {
      file_path: '/Users/dev/project/src/auth/validator.rs',
      content: 'pub struct TokenValidator { /* ... */ }',
    },
    toolUseID: 'tu_perm_003',
    timeoutMs: 60000,
    suggestions: [{ type: 'allow_tool', tool: 'Write' }],
  } satisfies PermissionRequest,
}

export const askQuestions = {
  singleSelect: {
    type: 'ask_question',
    requestId: 'q_001',
    questions: [
      {
        question: 'Which authentication strategy should I use for this service?',
        header: 'Authentication Strategy',
        options: [
          {
            label: 'JWT with RSA-256',
            description: 'Asymmetric keys, good for distributed systems',
          },
          { label: 'JWT with HMAC-256', description: 'Shared secret, simpler but less secure' },
          { label: 'OAuth 2.0 + PKCE', description: 'Full OAuth flow with code exchange' },
        ],
        multiSelect: false,
      },
    ],
  } satisfies AskQuestion,

  multiSelect: {
    type: 'ask_question',
    requestId: 'q_002',
    questions: [
      {
        question: 'Which files should I include in the refactoring scope?',
        header: 'Refactoring Scope',
        options: [
          { label: 'auth/middleware.rs', description: 'Main middleware handler (450 lines)' },
          { label: 'auth/session.rs', description: 'Session management (200 lines)' },
          { label: 'auth/types.rs', description: 'Shared types and traits (80 lines)' },
          { label: 'tests/auth_test.rs', description: 'Integration tests (300 lines)' },
        ],
        multiSelect: true,
      },
    ],
  } satisfies AskQuestion,
}

export const planApprovals = {
  simple: {
    type: 'plan_approval',
    requestId: 'plan_001',
    planData: {
      plan: `1. Extract TokenValidator into auth/validator.rs
2. Move session logic to auth/session.rs
3. Update middleware to compose both services
4. Add unit tests for TokenValidator
5. Run integration test suite`,
    },
  } satisfies PlanApproval,
}

export const elicitations = {
  simple: {
    type: 'elicitation',
    requestId: 'elic_001',
    toolName: 'configure_mcp',
    toolInput: { server: 'postgres' },
    prompt: 'Please provide the database connection string for the PostgreSQL MCP server:',
  } satisfies Elicitation,
}

// ── Interaction Blocks ──────────────────────────────────────────────────────

export const interactionBlocks = {
  permissionPending: {
    type: 'interaction',
    id: 'ib_001',
    variant: 'permission',
    requestId: 'perm_001',
    resolved: false,
    data: permissionRequests.bash,
  } satisfies InteractionBlock,

  permissionResolved: {
    type: 'interaction',
    id: 'ib_002',
    variant: 'permission',
    requestId: 'perm_002',
    resolved: true,
    data: permissionRequests.edit,
  } satisfies InteractionBlock,

  questionPending: {
    type: 'interaction',
    id: 'ib_003',
    variant: 'question',
    requestId: 'q_001',
    resolved: false,
    data: askQuestions.singleSelect,
  } satisfies InteractionBlock,

  planPending: {
    type: 'interaction',
    id: 'ib_004',
    variant: 'plan',
    requestId: 'plan_001',
    resolved: false,
    data: planApprovals.simple,
  } satisfies InteractionBlock,

  elicitationPending: {
    type: 'interaction',
    id: 'ib_005',
    variant: 'elicitation',
    requestId: 'elic_001',
    resolved: false,
    data: elicitations.simple,
  } satisfies InteractionBlock,
}

// ── Progress Blocks ────────────────────────────────────────────────────────

export const progressBlocks = {
  bash: {
    type: 'progress',
    id: 'pb_001',
    variant: 'bash',
    category: 'builtin',
    data: {
      type: 'bash',
      output: 'Compiling claude-view-core v0.23.0\n   Compiling claude-view-server v0.23.0',
      fullOutput:
        'Compiling claude-view-core v0.23.0\n   Compiling claude-view-server v0.23.0\n    Finished in 12.3s',
      elapsedTimeSeconds: 12.3,
      totalLines: 3,
      totalBytes: 128 as unknown as bigint,
    },
    ts: FIVE_MIN_AGO,
    parentToolUseId: 'tu_bash_001',
  } satisfies ProgressBlock,

  agent: {
    type: 'progress',
    id: 'pb_002',
    variant: 'agent',
    category: 'agent',
    data: {
      type: 'agent',
      prompt: 'Research authentication best practices for Axum middleware',
      agentId: 'agent_research_001',
    },
    ts: FIVE_MIN_AGO + 10,
  } satisfies ProgressBlock,

  hook: {
    type: 'progress',
    id: 'pb_003',
    variant: 'hook',
    category: 'hook',
    data: {
      type: 'hook',
      hookEvent: 'PreToolUse',
      hookName: 'live-monitor',
      command: '/Users/dev/.claude/hooks/pre-tool.sh',
      statusMessage: 'Validating tool use…',
    },
    ts: FIVE_MIN_AGO + 20,
  } satisfies ProgressBlock,

  mcp: {
    type: 'progress',
    id: 'pb_004',
    variant: 'mcp',
    category: 'mcp',
    data: {
      type: 'mcp',
      status: 'running',
      serverName: 'postgres',
      toolName: 'query',
    },
    ts: FIVE_MIN_AGO + 30,
  } satisfies ProgressBlock,

  taskQueue: {
    type: 'progress',
    id: 'pb_005',
    variant: 'task_queue',
    category: 'agent',
    data: {
      type: 'task_queue',
      taskDescription: 'Waiting for file lock on package cache',
      taskType: 'local_bash',
    },
    ts: FIVE_MIN_AGO + 40,
  } satisfies ProgressBlock,

  search: {
    type: 'progress',
    id: 'pb_006',
    variant: 'search',
    category: 'builtin',
    data: { type: 'search', resultCount: 14, query: 'JWT middleware Axum' },
    ts: FIVE_MIN_AGO + 50,
  } satisfies ProgressBlock,

  query: {
    type: 'progress',
    id: 'pb_007',
    variant: 'query',
    category: 'builtin',
    data: { type: 'query', query: 'SELECT * FROM sessions LIMIT 10' },
    ts: FIVE_MIN_AGO + 60,
  } satisfies ProgressBlock,
}

// ── Full Conversation ───────────────────────────────────────────────────────

export const fullConversation: ConversationBlock[] = [
  // Turn 1: basic Q&A
  userBlocks.normal,
  assistantBlocks.textOnly,
  turnBoundaryBlocks.success,

  // Turn 2: tool usage + progress
  userBlocks.sent,
  assistantBlocks.withTools,
  progressBlocks.bash,
  progressBlocks.hook,
  turnBoundaryBlocks.cheap,

  // Notices
  noticeBlocks.sessionResumed,
  noticeBlocks.contextCompacted,
  noticeBlocks.rateLimitWarning,

  // Turn 3: long prompt + thinking + markdown
  userBlocks.long,
  assistantBlocks.withThinking,
  assistantBlocks.markdown,
  turnBoundaryBlocks.success,

  // Interaction blocks (all variants)
  interactionBlocks.permissionPending,
  interactionBlocks.questionPending,
  interactionBlocks.planPending,
  interactionBlocks.elicitationPending,

  // System blocks
  systemBlocks.taskStarted,
  systemBlocks.taskProgress,
  systemBlocks.taskCompleted,

  // Progress blocks (all 7 variants)
  progressBlocks.bash,
  progressBlocks.agent,
  progressBlocks.hook,
  progressBlocks.mcp,
  progressBlocks.taskQueue,
  progressBlocks.search,
  progressBlocks.query,

  // Active state
  assistantBlocks.withRunningTool,
]

export const errorConversation: ConversationBlock[] = [
  userBlocks.normal,
  noticeBlocks.assistantError,
  noticeBlocks.rateLimitWarning,
  turnBoundaryBlocks.error,
  userBlocks.failed,
  noticeBlocks.fatalError,
]
