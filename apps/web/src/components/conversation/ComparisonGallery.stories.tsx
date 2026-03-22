/**
 * Comparison Gallery — side-by-side rendering of the SAME message across pipelines.
 *
 * Shows Developer (ConversationBlock) vs RichPane (RichMessage) vs ActionLog
 * for every overlapping message type so conflicts can be resolved one-by-one.
 *
 * Three columns:
 *   1. Developer  — ConversationBlock + developer registry (indigo)
 *   2. RichPane   — RichMessage live terminal renderer (emerald)
 *   3. ActionLog  — RichMessage → ActionItem timeline (amber)
 *
 * 39 comparison rows covering ALL message types:
 *   user, assistant (text/thinking/tools/streaming/markdown),
 *   interaction (permission/question/plan/elicitation),
 *   turn_boundary (success/error/max_turns),
 *   notice (error/rate_limit/auth/session/prompt_suggestion),
 *   system (all 17 variants), progress (all 7 variants),
 *   tool_use/result, error, hook
 */
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { Meta, StoryObj } from '@storybook/react-vite'
import { useEffect, useMemo } from 'react'
import { useMonitorStore } from '../../store/monitor-store'
import { withConversationActions } from '../../stories/decorators'
import {
  assistantBlocks,
  interactionBlocks,
  noticeBlocks,
  progressBlocks,
  systemBlocks,
  turnBoundaryBlocks,
  userBlocks,
} from '../../stories/fixtures'
import { devSystemBlocks } from '../../stories/fixtures-developer'
import type { RichMessage } from '../live/RichPane'
import { RichPane } from '../live/RichPane'
import { ActionFilterChips } from '../live/action-log/ActionFilterChips'
import { ActionLogTab } from '../live/action-log/ActionLogTab'
import type { ActionCategory } from '../live/action-log/types'
import { ConversationThread } from './ConversationThread'
import { developerRegistry } from './blocks/developer/registry'
import type { BlockRenderers } from './types'

// ── Shared timestamp ────────────────────────────────────────────────────────

const NOW = Math.floor(Date.now() / 1000)

// ── Comparison definitions ──────────────────────────────────────────────────

interface Comparison {
  label: string
  description: string
  devBlocks: ConversationBlock[]
  richMessages: RichMessage[]
  /** Height hint for RichPane Virtuoso container (px). Default 200. */
  paneHeight?: number
}

const comparisons: Comparison[] = [
  // ── 1. User ──
  {
    label: 'user',
    description: 'DevUserBlock (status dot + ID) vs RichPane UserMessage (blue border + timestamp)',
    devBlocks: [userBlocks.normal],
    richMessages: [
      {
        type: 'user',
        content: 'Can you help me refactor the authentication middleware?',
        ts: NOW - 300,
      },
    ],
  },

  // ── 2. Assistant (text only) ──
  {
    label: 'assistant (text)',
    description:
      'DevAssistantBlock (segments + metadata) vs RichPane AssistantMessage (markdown + timestamp)',
    devBlocks: [assistantBlocks.textOnly],
    richMessages: [
      {
        type: 'assistant',
        content:
          "I'll help you refactor the authentication middleware. Let me start by reading the current implementation to understand the structure.",
        ts: NOW - 295,
      },
    ],
  },

  // ── 3. Thinking ──
  {
    label: 'thinking',
    description:
      'Embedded in DevAssistantBlock.thinking vs standalone RichPane ThinkingMessage (collapsible)',
    devBlocks: [assistantBlocks.withThinking],
    richMessages: [
      {
        type: 'thinking',
        content:
          'The user wants to refactor auth middleware. Let me think about the best approach...\n\nThe current code has tight coupling between token validation and session management.',
        ts: NOW - 290,
      },
    ],
    paneHeight: 150,
  },

  // ── 4. Tool use + result pair ──
  {
    label: 'tool_use + tool_result',
    description:
      'ToolCard nested in DevAssistantBlock vs RichPane PairedToolCard (side-by-side pair)',
    devBlocks: [assistantBlocks.withTools],
    richMessages: [
      {
        type: 'tool_use',
        content: 'Read',
        name: 'Read',
        input: '{"file_path": "src/auth/middleware.rs"}',
        inputData: { file_path: 'src/auth/middleware.rs' },
        ts: NOW - 280,
        category: 'builtin',
      },
      {
        type: 'tool_result',
        content: 'fn main() {\n    println!("Hello, world!");\n}',
        name: 'Read',
        ts: NOW - 279,
        category: 'builtin',
      },
      {
        type: 'tool_use',
        content: 'Edit',
        name: 'Edit',
        input:
          '{"file_path": "src/auth/middleware.rs", "old_string": "fn validate(&self)", "new_string": "fn validate(&mut self)"}',
        inputData: {
          file_path: 'src/auth/middleware.rs',
          old_string: 'fn validate(&self)',
          new_string: 'fn validate(&mut self)',
        },
        ts: NOW - 270,
        category: 'builtin',
      },
      {
        type: 'tool_result',
        content: 'Successfully edited file',
        name: 'Edit',
        ts: NOW - 269,
        category: 'builtin',
      },
    ],
    paneHeight: 400,
  },

  // ── 5. Error ──
  {
    label: 'error',
    description:
      'DevNoticeBlock (AlertCircle + variant) vs RichPane ErrorMessage (red border + JSON/text)',
    devBlocks: [noticeBlocks.error as ConversationBlock],
    richMessages: [
      {
        type: 'error',
        content: 'WebSocket connection lost. Attempting to reconnect...',
        ts: NOW - 260,
        category: 'error',
      },
    ],
  },

  // ── 6. Hook ──
  {
    label: 'hook',
    description:
      'DevSystemBlock (hook_event variant) vs RichPane HookMessage (amber dot + expandable)',
    devBlocks: [devSystemBlocks.hookEvent as ConversationBlock],
    richMessages: [
      {
        type: 'hook',
        content: 'PreToolUse:Bash — live-monitor: validating',
        ts: NOW - 250,
        category: 'hook',
        metadata: { type: 'hook_progress', hookName: 'live-monitor', hookEvent: 'PreToolUse' },
      },
    ],
  },

  // ── 7. System: queue_operation ──
  {
    label: 'system: queue_operation',
    description: 'DevSystemBlock(queue_operation) vs RichPane SystemMessageCard(queue-operation)',
    devBlocks: [devSystemBlocks.queueOperation as ConversationBlock],
    richMessages: [
      {
        type: 'system',
        content: 'queue-enqueue: Fix login bug',
        ts: NOW - 240,
        category: 'queue',
        metadata: { type: 'queue-operation', operation: 'enqueue' },
      },
    ],
  },

  // ── 8. System: file_history_snapshot ──
  {
    label: 'system: file_history_snapshot',
    description:
      'DevSystemBlock(file_history_snapshot) vs RichPane SystemMessageCard(file-history-snapshot)',
    devBlocks: [devSystemBlocks.fileHistorySnapshot as ConversationBlock],
    richMessages: [
      {
        type: 'system',
        content: 'file-history-snapshot',
        ts: NOW - 230,
        category: 'snapshot',
        metadata: { type: 'file-history-snapshot' },
      },
    ],
  },

  // ── 9. System: local_command ──
  {
    label: 'system: local_command',
    description: 'DevSystemBlock(local_command) vs RichPane SystemMessageCard(local_command)',
    devBlocks: [devSystemBlocks.localCommand as ConversationBlock],
    richMessages: [
      {
        type: 'system',
        content: '/clear',
        ts: NOW - 220,
        category: 'system',
        metadata: { type: 'local_command', content: '/clear' },
      },
    ],
  },

  // ── 10. System: compact_boundary ──
  {
    label: 'system: compact_boundary',
    description:
      'DevNoticeBlock(context_compacted) vs RichPane SystemMessageCard(compact_boundary) — DIFFERENT block types!',
    devBlocks: [noticeBlocks.contextCompacted as ConversationBlock],
    richMessages: [
      {
        type: 'system',
        content: 'Context compacted',
        ts: NOW - 210,
        category: 'system',
        metadata: { type: 'compact_boundary', trigger: 'auto', preTokens: 145000 },
      },
    ],
  },

  // ── 11. Progress: bash ──
  {
    label: 'progress: bash',
    description:
      'DevProgressBlock(bash) vs RichPane ProgressMessageCard(bash_progress) — variant name mismatch',
    devBlocks: [progressBlocks.bash as ConversationBlock],
    richMessages: [
      {
        type: 'progress',
        content: 'bash_progress: Compiling...',
        ts: NOW - 200,
        category: 'builtin',
        metadata: {
          type: 'bash_progress',
          output: 'Compiling claude-view v0.23.0',
          elapsedTimeSeconds: 12.3,
        },
      },
    ],
  },

  // ── 12. Progress: agent ──
  {
    label: 'progress: agent',
    description: 'DevProgressBlock(agent) vs RichPane ProgressMessageCard(agent_progress)',
    devBlocks: [progressBlocks.agent as ConversationBlock],
    richMessages: [
      {
        type: 'progress',
        content: 'agent_progress: Research',
        ts: NOW - 190,
        category: 'agent',
        metadata: { type: 'agent_progress', agentId: 'agent_001', prompt: 'Research auth' },
      },
    ],
  },

  // ── 13. Progress: hook ──
  {
    label: 'progress: hook',
    description: 'DevProgressBlock(hook) vs RichPane ProgressMessageCard(hook_progress)',
    devBlocks: [progressBlocks.hook as ConversationBlock],
    richMessages: [
      {
        type: 'progress',
        content: 'hook_progress: pre-commit',
        ts: NOW - 180,
        category: 'hook',
        metadata: {
          type: 'hook_progress',
          hookName: 'pre-commit',
          hookEvent: 'PreToolUse',
          command: 'hooks/pre-tool.sh',
        },
      },
    ],
  },

  // ── 14. Progress: mcp ──
  {
    label: 'progress: mcp',
    description: 'DevProgressBlock(mcp) vs RichPane ProgressMessageCard(mcp_progress)',
    devBlocks: [progressBlocks.mcp as ConversationBlock],
    richMessages: [
      {
        type: 'progress',
        content: 'mcp_progress: postgres/query',
        ts: NOW - 170,
        category: 'mcp',
        metadata: {
          type: 'mcp_progress',
          serverName: 'postgres',
          toolName: 'query',
          status: 'running',
        },
      },
    ],
  },

  // ═══════════════════════════════════════════════════════════════════════════
  // Developer-only types (no RichPane/ActionLog equivalent)
  // ═══════════════════════════════════════════════════════════════════════════

  // ── 15. Interaction: permission (pending) ──
  {
    label: 'interaction: permission (pending)',
    description:
      'DevInteractionBlock → PermissionCard vs RichPane tool_use (Bash needing permission)',
    devBlocks: [interactionBlocks.permissionPending as ConversationBlock],
    richMessages: [
      {
        type: 'tool_use',
        content: 'Bash',
        name: 'Bash',
        input: '{"command": "cargo test --workspace -- --nocapture"}',
        inputData: { command: 'cargo test --workspace -- --nocapture' },
        ts: NOW - 165,
        category: 'builtin',
      },
    ],
    paneHeight: 250,
  },

  // ── 16. Interaction: permission (resolved) ──
  {
    label: 'interaction: permission (resolved)',
    description:
      'DevInteractionBlock → PermissionCard resolved vs RichPane tool_use + tool_result pair',
    devBlocks: [interactionBlocks.permissionResolved as ConversationBlock],
    richMessages: [
      {
        type: 'tool_use',
        content: 'Edit',
        name: 'Edit',
        input:
          '{"file_path": "/Users/dev/project/src/auth/middleware.rs", "old_string": "fn validate_token(&self)", "new_string": "fn validate_token(&mut self)"}',
        inputData: {
          file_path: '/Users/dev/project/src/auth/middleware.rs',
          old_string: 'fn validate_token(&self)',
          new_string: 'fn validate_token(&mut self)',
        },
        ts: NOW - 163,
        category: 'builtin',
      },
      {
        type: 'tool_result',
        content: 'Successfully edited file',
        name: 'Edit',
        ts: NOW - 162,
        category: 'builtin',
      },
    ],
    paneHeight: 300,
  },

  // ── 17. Interaction: question ──
  {
    label: 'interaction: question',
    description:
      'DevInteractionBlock → AskUserQuestionCard vs RichPane AskUserQuestionDisplay (amber, via tool_use)',
    devBlocks: [interactionBlocks.questionPending as ConversationBlock],
    richMessages: [
      {
        type: 'tool_use',
        content: 'AskUserQuestion',
        name: 'AskUserQuestion',
        input: JSON.stringify({
          questions: [
            {
              question: 'Which authentication strategy should I use for this service?',
              header: 'Authentication Strategy',
              options: [
                {
                  label: 'JWT with RSA-256',
                  description: 'Asymmetric keys, good for distributed systems',
                },
                {
                  label: 'JWT with HMAC-256',
                  description: 'Shared secret, simpler but less secure',
                },
                { label: 'OAuth 2.0 + PKCE', description: 'Full OAuth flow with code exchange' },
              ],
              multiSelect: false,
            },
          ],
        }),
        inputData: {
          questions: [
            {
              question: 'Which authentication strategy should I use for this service?',
              header: 'Authentication Strategy',
              options: [
                {
                  label: 'JWT with RSA-256',
                  description: 'Asymmetric keys, good for distributed systems',
                },
                {
                  label: 'JWT with HMAC-256',
                  description: 'Shared secret, simpler but less secure',
                },
                { label: 'OAuth 2.0 + PKCE', description: 'Full OAuth flow with code exchange' },
              ],
              multiSelect: false,
            },
          ],
        },
        ts: NOW - 160,
        category: 'builtin',
      },
    ],
    paneHeight: 350,
  },

  // ── 18. Interaction: plan ──
  {
    label: 'interaction: plan',
    description:
      'DevInteractionBlock → PlanApprovalCard vs RichPane PlanApprovalCard (via ExitPlanMode tool_use)',
    devBlocks: [interactionBlocks.planPending as ConversationBlock],
    richMessages: [
      {
        type: 'tool_use',
        content: 'ExitPlanMode',
        name: 'ExitPlanMode',
        input: JSON.stringify({
          plan: '1. Extract TokenValidator into auth/validator.rs\n2. Move session logic to auth/session.rs\n3. Update middleware\n4. Add unit tests\n5. Run integration suite',
        }),
        inputData: {
          plan: '1. Extract TokenValidator into auth/validator.rs\n2. Move session logic to auth/session.rs\n3. Update middleware\n4. Add unit tests\n5. Run integration suite',
        },
        ts: NOW - 155,
        category: 'builtin',
      },
    ],
    paneHeight: 300,
  },

  // ── 19. Interaction: elicitation ──
  {
    label: 'interaction: elicitation',
    description: 'DevInteractionBlock → ElicitationCard vs RichPane tool_use (MCP elicitation)',
    devBlocks: [interactionBlocks.elicitationPending as ConversationBlock],
    richMessages: [
      {
        type: 'tool_use',
        content: 'configure_mcp',
        name: 'mcp__postgres__configure',
        input: '{"server": "postgres"}',
        inputData: { server: 'postgres' },
        ts: NOW - 152,
        category: 'mcp',
      },
    ],
    paneHeight: 200,
  },

  // ── 20. Turn boundary: success (+ RichPane turn_duration system event) ──
  {
    label: 'turn_boundary: success',
    description: 'DevTurnBoundary vs RichPane SystemMessageCard(turn_duration)',
    devBlocks: [turnBoundaryBlocks.success as ConversationBlock],
    richMessages: [
      {
        type: 'system',
        content: 'Turn completed in 12.5s',
        ts: NOW - 150,
        metadata: { type: 'turn_duration', durationMs: 12500 },
      },
    ],
  },

  // ── 21. Turn boundary: error ──
  {
    label: 'turn_boundary: error',
    description: 'DevTurnBoundary with error vs RichPane ErrorMessage',
    devBlocks: [turnBoundaryBlocks.error as ConversationBlock],
    richMessages: [
      {
        type: 'error',
        content: 'Tool execution failed: ENOENT /tmp/missing.txt',
        ts: NOW - 148,
        category: 'error',
      },
    ],
  },

  // ── 22. Turn boundary: max turns ──
  {
    label: 'turn_boundary: max_turns',
    description: 'DevTurnBoundary exceeded max turns vs RichPane ErrorMessage',
    devBlocks: [turnBoundaryBlocks.maxTurns as ConversationBlock],
    richMessages: [
      {
        type: 'error',
        content: 'Reached maximum number of turns (25)',
        ts: NOW - 146,
        category: 'error',
      },
    ],
  },

  // ── 23. Notice: assistant_error ──
  {
    label: 'notice: assistant_error',
    description: 'DevNoticeBlock assistant error variants',
    devBlocks: [
      noticeBlocks.assistantError as ConversationBlock,
      noticeBlocks.billingError as ConversationBlock,
      noticeBlocks.authFailed as ConversationBlock,
      noticeBlocks.serverError as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 24. Notice: rate_limit ──
  {
    label: 'notice: rate_limit',
    description: 'DevNoticeBlock rate limit (warning + rejected)',
    devBlocks: [
      noticeBlocks.rateLimitWarning as ConversationBlock,
      noticeBlocks.rateLimitRejected as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 25. Notice: auth_status ──
  {
    label: 'notice: auth_status',
    description: 'DevNoticeBlock auth (authenticating + error)',
    devBlocks: [
      noticeBlocks.authenticating as ConversationBlock,
      noticeBlocks.authError as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 26. Notice: session_closed + resumed ──
  {
    label: 'notice: session lifecycle',
    description: 'DevNoticeBlock session closed + resumed',
    devBlocks: [
      noticeBlocks.sessionClosed as ConversationBlock,
      noticeBlocks.sessionResumed as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 27. Notice: prompt_suggestion ──
  {
    label: 'notice: prompt_suggestion',
    description: 'DevNoticeBlock prompt suggestion (clickable pill)',
    devBlocks: [noticeBlocks.promptSuggestion as ConversationBlock],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 28. Notice: fatal error ──
  {
    label: 'notice: fatal_error',
    description: 'DevNoticeBlock fatal vs non-fatal error',
    devBlocks: [
      noticeBlocks.error as ConversationBlock,
      noticeBlocks.fatalError as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 29a. System: api_error (RichPane-only) ──
  {
    label: 'system: api_error',
    description:
      'RichPane SystemMessageCard(api_error) — no direct Developer equivalent (notice handles it)',
    devBlocks: [],
    richMessages: [
      {
        type: 'system',
        content: 'API error',
        ts: NOW - 148,
        metadata: {
          type: 'api_error',
          error: 'Rate limit exceeded. Please retry after 30s.',
          retryAttempt: 2,
          maxRetries: 5,
          retryInMs: 5000,
        },
      },
    ],
  },

  // ── 29b. System: hook_summary (RichPane-only) ──
  {
    label: 'system: hook_summary',
    description: 'RichPane SystemMessageCard(hook_summary) — no direct Developer equivalent',
    devBlocks: [],
    richMessages: [
      {
        type: 'system',
        content: 'Hook summary',
        ts: NOW - 146,
        metadata: {
          type: 'hook_summary',
          hookCount: 3,
          hookInfos: ['pre-commit (120ms)', 'lint (450ms)'],
          hookErrors: [],
          durationMs: 570,
        },
      },
    ],
  },

  // ── 29. System: session_init ──
  {
    label: 'system: session_init',
    description: 'DevSystemBlock session init — model, tools, cwd',
    devBlocks: [devSystemBlocks.sessionInit as ConversationBlock],
    richMessages: [],
  },

  // ── 30. System: session_status ──
  {
    label: 'system: session_status',
    description: 'DevSystemBlock session status (compacting + idle)',
    devBlocks: [
      devSystemBlocks.sessionStatus as ConversationBlock,
      devSystemBlocks.sessionStatusIdle as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 31. System: hook_event (error) ──
  {
    label: 'system: hook_event (error)',
    description: 'DevSystemBlock hook event with error outcome',
    devBlocks: [devSystemBlocks.hookEventError as ConversationBlock],
    richMessages: [],
  },

  // ── 32. System: task lifecycle ──
  {
    label: 'system: task lifecycle',
    description:
      'Task started → progress → completed → failed vs RichPane progress(agent_progress)',
    devBlocks: [
      systemBlocks.taskStarted as ConversationBlock,
      systemBlocks.taskProgress as ConversationBlock,
      systemBlocks.taskCompleted as ConversationBlock,
      systemBlocks.taskFailed as ConversationBlock,
    ],
    richMessages: [
      {
        type: 'progress',
        content: 'agent_progress',
        ts: NOW - 125,
        category: 'agent',
        metadata: {
          type: 'agent_progress',
          agentId: 'task_001',
          prompt: 'Researching authentication best practices',
        },
      },
      {
        type: 'assistant',
        content: 'Research complete — found 5 patterns for JWT middleware in Axum',
        ts: NOW - 120,
      },
    ],
    paneHeight: 250,
  },

  // ── 33. System: files_saved ──
  {
    label: 'system: files_saved',
    description: 'DevSystemBlock files saved (success + with failures)',
    devBlocks: [
      devSystemBlocks.filesSaved as ConversationBlock,
      devSystemBlocks.filesSavedWithFailures as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 34. System: command_output + stream_delta ──
  {
    label: 'system: output events',
    description: 'DevSystemBlock command output + stream delta + unknown',
    devBlocks: [
      devSystemBlocks.commandOutput as ConversationBlock,
      devSystemBlocks.streamDelta as ConversationBlock,
      devSystemBlocks.unknown as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 35. System: metadata events ──
  {
    label: 'system: metadata',
    description: 'DevSystemBlock ai_title + last_prompt + informational',
    devBlocks: [
      devSystemBlocks.aiTitle as ConversationBlock,
      devSystemBlocks.lastPrompt as ConversationBlock,
      devSystemBlocks.informational as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 36. Progress: task_queue + search + query ──
  {
    label: 'progress: other variants',
    description:
      'DevProgressBlock task_queue + search + query vs RichPane waiting_for_task + hook_event',
    devBlocks: [
      progressBlocks.taskQueue as ConversationBlock,
      progressBlocks.search as ConversationBlock,
      progressBlocks.query as ConversationBlock,
    ],
    richMessages: [
      {
        type: 'progress',
        content: 'waiting_for_task',
        ts: NOW - 135,
        category: 'queue' as ActionCategory,
        metadata: { type: 'waiting_for_task', position: 3, queueLength: 5, waitDuration: 12000 },
      },
      {
        type: 'progress',
        content: 'hook_event',
        ts: NOW - 130,
        category: 'hook' as ActionCategory,
        metadata: {
          type: 'hook_event',
          _hookEvent: {
            eventName: 'PreToolUse',
            label: 'Validating Bash command',
            toolName: 'live-monitor',
            group: 'autonomous',
          },
        },
      },
    ],
    paneHeight: 250,
  },

  // ── 37. Assistant: all tool variants ──
  {
    label: 'assistant: all tool variants',
    description: 'DevAssistantBlock with diff, JSON, MCP, agent tools',
    devBlocks: [assistantBlocks.withAllToolVariants as ConversationBlock],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 38. Assistant: streaming ──
  {
    label: 'assistant: streaming',
    description: 'DevAssistantBlock streaming cursor + running tool',
    devBlocks: [
      assistantBlocks.streaming as ConversationBlock,
      assistantBlocks.withRunningTool as ConversationBlock,
    ],
    richMessages: [],
    paneHeight: 100,
  },

  // ── 39. Assistant: markdown ──
  {
    label: 'assistant: markdown',
    description: 'DevAssistantBlock rich markdown (tables, code, lists)',
    devBlocks: [assistantBlocks.markdown as ConversationBlock],
    richMessages: [],
    paneHeight: 100,
  },
]

// ── Comparison Row ──────────────────────────────────────────────────────────

/** Map a RichMessage to its ActionCategory for counting purposes. */
function messageToCategory(msg: RichMessage): ActionCategory {
  if (msg.category) return msg.category as ActionCategory
  switch (msg.type) {
    case 'user':
    case 'assistant':
      return 'system'
    case 'thinking':
      return 'system'
    case 'tool_use':
    case 'tool_result':
      return 'builtin'
    case 'error':
      return 'error'
    case 'hook':
      return 'hook'
    case 'system':
      return 'system'
    case 'progress':
      return 'system'
    default:
      return 'system'
  }
}

/** Compute ActionCategory counts from RichMessage[] — counts every message. */
function computeCounts(messages: RichMessage[]): Record<ActionCategory, number> {
  const c: Record<ActionCategory, number> = {
    skill: 0,
    mcp: 0,
    builtin: 0,
    agent: 0,
    error: 0,
    hook: 0,
    hook_progress: 0,
    system: 0,
    snapshot: 0,
    queue: 0,
  }
  for (const msg of messages) {
    c[messageToCategory(msg)]++
  }
  return c
}

function ComparisonRow({
  label,
  description,
  devBlocks,
  richMessages,
  paneHeight = 200,
}: Comparison) {
  const counts = useMemo(() => computeCounts(richMessages), [richMessages])

  return (
    <section className="border border-gray-700 rounded-lg overflow-hidden">
      {/* Row header */}
      <div className="bg-gray-800 px-4 py-2 border-b border-gray-700">
        <h2 className="text-sm font-bold text-white font-mono">{label}</h2>
        <p className="text-[11px] text-gray-400 mt-0.5">{description}</p>
      </div>

      {/* Three-column comparison */}
      <div className="grid grid-cols-3 divide-x divide-gray-700">
        {/* Developer */}
        <div className="p-3">
          <div className="text-[10px] font-bold uppercase tracking-wider text-indigo-400 mb-2">
            Developer (ConversationBlock)
          </div>
          <ActionFilterChips counts={counts} activeFilter="all" onFilterChange={() => {}} />
          <ConversationThread blocks={devBlocks} renderers={developerRegistry} />
        </div>

        {/* RichPane — has internal ActionFilterChips, pass categoryCounts */}
        <div className="p-3">
          <div className="text-[10px] font-bold uppercase tracking-wider text-emerald-400 mb-2">
            RichPane (RichMessage)
          </div>
          <div style={{ height: paneHeight }}>
            <RichPane
              messages={richMessages}
              isVisible={true}
              verboseMode={true}
              bufferDone={true}
              categoryCounts={counts}
            />
          </div>
        </div>

        {/* ActionLog — has internal ActionFilterChips, pass categoryCounts */}
        <div className="p-3">
          <div className="text-[10px] font-bold uppercase tracking-wider text-amber-400 mb-2">
            ActionLog (RichMessage → ActionItem)
          </div>
          <div style={{ height: paneHeight }}>
            <ActionLogTab messages={richMessages} bufferDone={true} categoryCounts={counts} />
          </div>
        </div>
      </div>
    </section>
  )
}

// ── Gallery ─────────────────────────────────────────────────────────────────

function ComparisonGallery(_props: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
}) {
  // Reset RichPane's persisted state so every message type is visible in rich mode
  const setVerboseFilter = useMonitorStore((s) => s.setVerboseFilter)
  const setRichRenderMode = useMonitorStore((s) => s.setRichRenderMode)
  useEffect(() => {
    setVerboseFilter('all')
    setRichRenderMode('rich')
  }, [setVerboseFilter, setRichRenderMode])

  return (
    <div className="w-full px-4 py-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4 text-xs text-gray-400 border-b border-gray-700 pb-3">
        <span className="font-bold text-sm text-white">
          Pipeline Comparison — Developer vs RichPane vs ActionLog
        </span>
        <span>{comparisons.length} overlapping types</span>
        <span className="text-indigo-400">Developer</span>
        <span className="text-emerald-400">RichPane</span>
        <span className="text-amber-400">ActionLog</span>
      </div>

      {comparisons.map((comp) => (
        <ComparisonRow key={comp.label} {...comp} />
      ))}
    </div>
  )
}

// ── Meta ─────────────────────────────────────────────────────────────────────

const meta = {
  title: 'Gallery/Comparison',
  component: ComparisonGallery,
  decorators: [withConversationActions],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ComparisonGallery>

export default meta
type Story = StoryObj<typeof meta>

/** Side-by-side comparison of ALL 39 message types across pipelines. */
export const PipelineComparison: Story = {
  args: { blocks: [], renderers: developerRegistry },
}
