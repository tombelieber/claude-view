/**
 * Gallery — THE single source of truth for all conversation UI.
 * Renders ALL 7 block types × ALL variants in one scrollable view,
 * PLUS the RichPane pipeline for completeness.
 *
 * Display modes:
 *   1. Chat       — clean bubble UI for end users
 *   2. Developer  — rich detail cards (use { } toggle for JSON mode)
 *   3. RichPane   — Live Monitor terminal view (RichMessage[] pipeline)
 */
import type { ConversationBlock } from '@claude-view/shared/types/blocks'
import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../stories/decorators'
import {
  assistantBlocks,
  interactionBlocks,
  noticeBlocks,
  progressBlocks,
  systemBlocks,
  turnBoundaryBlocks,
  userBlocks,
} from '../../stories/fixtures'
import {
  devAssistantBlocks,
  devSystemBlocks,
  devSystemBlocksWithRawJson,
  devTurnBoundaryBlocks,
  devUserBlocks,
} from '../../stories/fixtures-developer'
import type { RichMessage } from '../live/RichPane'
import { RichPane } from '../live/RichPane'
import { ActionLogTab } from '../live/action-log/ActionLogTab'
import { ConversationThread } from './ConversationThread'
import { chatRegistry } from './blocks/chat/registry'
import { developerRegistry } from './blocks/developer/registry'
import type { BlockRenderers } from './types'

// ── Every single variant, grouped by block type ─────────────────────────────

const allBlocks: ConversationBlock[] = [
  // ── UserBlock (6 variants) ──
  userBlocks.normal,
  userBlocks.sent,
  userBlocks.optimistic,
  userBlocks.sending,
  userBlocks.failed,
  userBlocks.long,

  // ── AssistantBlock (8 variants) ──
  assistantBlocks.textOnly,
  assistantBlocks.streaming,
  assistantBlocks.withThinking,
  assistantBlocks.withTools,
  assistantBlocks.withRunningTool,
  assistantBlocks.withToolError,
  assistantBlocks.markdown,
  assistantBlocks.withAllToolVariants,

  // ── InteractionBlock (5 variants) ──
  interactionBlocks.permissionPending,
  interactionBlocks.permissionResolved,
  interactionBlocks.questionPending,
  interactionBlocks.planPending,
  interactionBlocks.elicitationPending,

  // ── TurnBoundaryBlock (5 variants) ──
  turnBoundaryBlocks.success,
  turnBoundaryBlocks.cheap,
  turnBoundaryBlocks.free,
  turnBoundaryBlocks.error,
  turnBoundaryBlocks.maxTurns,

  // ── NoticeBlock (15 variants) ──
  noticeBlocks.assistantError,
  noticeBlocks.billingError,
  noticeBlocks.authFailed,
  noticeBlocks.serverError,
  noticeBlocks.rateLimitWarning,
  noticeBlocks.rateLimitRejected,
  noticeBlocks.contextCompacted,
  noticeBlocks.contextCompactedManual,
  noticeBlocks.authenticating,
  noticeBlocks.authError,
  noticeBlocks.sessionClosed,
  noticeBlocks.error,
  noticeBlocks.fatalError,
  noticeBlocks.promptSuggestion,
  noticeBlocks.sessionResumed,

  // ── SystemBlock (17 variants) ──
  devSystemBlocks.sessionInit,
  devSystemBlocks.sessionStatus,
  devSystemBlocks.sessionStatusIdle,
  devSystemBlocks.elicitationComplete,
  devSystemBlocks.hookEvent,
  devSystemBlocks.hookEventError,
  systemBlocks.taskStarted,
  systemBlocks.taskProgress,
  systemBlocks.taskCompleted,
  systemBlocks.taskFailed,
  devSystemBlocks.filesSaved,
  devSystemBlocks.filesSavedWithFailures,
  devSystemBlocks.commandOutput,
  devSystemBlocks.streamDelta,
  devSystemBlocks.unknown,
  devSystemBlocks.localCommand,
  devSystemBlocks.queueOperation,
  devSystemBlocks.fileHistorySnapshot,
  devSystemBlocks.aiTitle,
  devSystemBlocks.lastPrompt,
  devSystemBlocks.informational,

  // ── ProgressBlock (7 variants) ──
  progressBlocks.bash,
  progressBlocks.agent,
  progressBlocks.hook,
  progressBlocks.mcp,
  progressBlocks.taskQueue,
  progressBlocks.search,
  progressBlocks.query,
]

// Developer blocks with rawJson (extra detail panels)
const devEnrichedBlocks: ConversationBlock[] = [
  devUserBlocks.withRawJson,
  devUserBlocks.withImagePastes,
  devAssistantBlocks.withRawJson,
  devAssistantBlocks.withPermissionMode,
  devSystemBlocksWithRawJson.withRetry,
  devSystemBlocksWithRawJson.withApiError,
  devSystemBlocksWithRawJson.withHooks,
  devSystemBlocksWithRawJson.withAll,
  devTurnBoundaryBlocks.withPermissionDenials as ConversationBlock,
]

// ── Gallery layout ──────────────────────────────────────────────────────────

function BlockGallery({
  blocks,
  renderers,
  compact,
  title,
  filterBar,
  defaultJsonMode,
  actionLogMessages,
}: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
  title?: string
  filterBar?: boolean
  defaultJsonMode?: boolean
  actionLogMessages?: RichMessage[]
}) {
  const groups: { type: string; blocks: ConversationBlock[] }[] = []
  let current: { type: string; blocks: ConversationBlock[] } | null = null

  for (const block of blocks) {
    if (!current || current.type !== block.type) {
      current = { type: block.type, blocks: [] }
      groups.push(current)
    }
    current.blocks.push(block)
  }

  const totalVariants = blocks.length

  return (
    <div className="max-w-4xl mx-auto p-6 space-y-8">
      {/* Stats header */}
      <div className="flex items-center gap-4 text-xs text-gray-400 dark:text-gray-500 border-b border-gray-200 dark:border-gray-700 pb-3">
        {title && (
          <span className="font-bold text-sm text-gray-700 dark:text-gray-300">{title}</span>
        )}
        <span>{groups.length} block types</span>
        <span>{totalVariants} total variants</span>
      </div>

      {groups.map((group) => (
        <section key={group.type}>
          <h2 className="text-sm font-bold uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-3 border-b border-gray-200 dark:border-gray-700 pb-1">
            {group.type} ({group.blocks.length})
          </h2>
          <div style={{ height: Math.max(300, group.blocks.length * 120) }}>
            <ConversationThread
              blocks={group.blocks}
              renderers={renderers}
              compact={compact}
              filterBar={filterBar}
              defaultJsonMode={defaultJsonMode}
            />
          </div>
        </section>
      ))}

      {/* ActionLog — merged from Live Monitor pipeline */}
      {actionLogMessages && actionLogMessages.length > 0 && (
        <section>
          <h2 className="text-sm font-bold uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-3 border-b border-gray-200 dark:border-gray-700 pb-1">
            action_log ({actionLogMessages.length} messages)
          </h2>
          <div style={{ height: 500 }}>
            <ActionLogTab messages={actionLogMessages} bufferDone={true} />
          </div>
        </section>
      )}
    </div>
  )
}

// ── RichMessage fixtures (Live Monitor pipeline) ─────────────────────────────
//
// RichMessage[] is the Live Monitor's streaming pipeline (separate from ConversationBlock).
// ActionLog is merged into DeveloperRich/DeveloperRichWithRawJson;
// RichPaneView remains standalone as it's a different rendering mode.

const NOW_SECS = Math.floor(Date.now() / 1000)

const richPaneMessages: RichMessage[] = [
  { type: 'user', content: 'Refactor the auth middleware', ts: NOW_SECS - 300 },
  {
    type: 'thinking',
    content: 'Let me analyze the current middleware structure...',
    ts: NOW_SECS - 298,
  },
  {
    type: 'tool_use',
    content: 'Read',
    name: 'Read',
    input: '{"file_path": "src/auth/middleware.rs"}',
    inputData: { file_path: 'src/auth/middleware.rs' },
    ts: NOW_SECS - 295,
    category: 'builtin',
  },
  {
    type: 'tool_result',
    content:
      'pub fn validate_token(&self, token: &str) -> Result<Claims> {\n    // ... 450 lines\n}',
    name: 'Read',
    ts: NOW_SECS - 294,
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
    ts: NOW_SECS - 280,
    category: 'builtin',
  },
  {
    type: 'tool_result',
    content:
      'diff --git a/src/auth/middleware.rs\n--- a/src/auth/middleware.rs\n+++ b/src/auth/middleware.rs\n@@ -42,7 +42,7 @@\n-    fn validate(&self)\n+    fn validate(&mut self)',
    name: 'Edit',
    ts: NOW_SECS - 279,
    category: 'builtin',
  },
  {
    type: 'tool_use',
    content: 'mcp__postgres__query',
    name: 'mcp__postgres__query',
    input: '{"query": "SELECT count(*) FROM sessions"}',
    inputData: { query: 'SELECT count(*) FROM sessions' },
    ts: NOW_SECS - 270,
    category: 'mcp',
  },
  {
    type: 'tool_result',
    content: '{"count": 1463}',
    name: 'mcp__postgres__query',
    ts: NOW_SECS - 269,
    category: 'mcp',
  },
  {
    type: 'tool_use',
    content: 'Skill',
    name: 'Skill',
    input: '{"skill": "commit"}',
    inputData: { skill: 'commit' },
    ts: NOW_SECS - 260,
    category: 'skill',
  },
  {
    type: 'tool_use',
    content: 'Agent',
    name: 'Agent',
    input: '{"prompt": "Research auth patterns"}',
    inputData: { prompt: 'Research auth patterns' },
    ts: NOW_SECS - 250,
    category: 'agent',
  },
  {
    type: 'hook',
    content: 'PreToolUse:Bash — live-monitor: validating',
    ts: NOW_SECS - 240,
    category: 'hook',
    metadata: { type: 'hook_progress', hookName: 'live-monitor', hookEvent: 'PreToolUse' },
  },
  {
    type: 'progress',
    content: 'bash_progress: Compiling...',
    ts: NOW_SECS - 235,
    category: 'builtin',
    metadata: {
      type: 'bash_progress',
      output: 'Compiling claude-view v0.23.0',
      elapsedTimeSeconds: 12.3,
    },
  },
  {
    type: 'progress',
    content: 'agent_progress: Research',
    ts: NOW_SECS - 230,
    category: 'agent',
    metadata: { type: 'agent_progress', agentId: 'agent_001', prompt: 'Research auth' },
  },
  {
    type: 'progress',
    content: 'hook_progress: pre-commit',
    ts: NOW_SECS - 225,
    category: 'hook',
    metadata: {
      type: 'hook_progress',
      hookName: 'pre-commit',
      hookEvent: 'PreToolUse',
      command: 'hooks/pre-tool.sh',
    },
  },
  {
    type: 'progress',
    content: 'mcp_progress: postgres/query',
    ts: NOW_SECS - 220,
    category: 'mcp',
    metadata: {
      type: 'mcp_progress',
      serverName: 'postgres',
      toolName: 'query',
      status: 'running',
    },
  },
  {
    type: 'system',
    content: 'Turn completed in 12.5s',
    ts: NOW_SECS - 215,
    metadata: { type: 'turn_duration', durationMs: 12500 },
  },
  {
    type: 'system',
    content: 'Context compacted',
    ts: NOW_SECS - 210,
    category: 'system',
    metadata: { type: 'compact_boundary', trigger: 'auto', preTokens: 145000 },
  },
  {
    type: 'system',
    content: 'queue-enqueue: Fix login bug',
    ts: NOW_SECS - 205,
    category: 'queue',
    metadata: { type: 'queue-operation', operation: 'enqueue' },
  },
  {
    type: 'system',
    content: 'file-history-snapshot',
    ts: NOW_SECS - 200,
    category: 'snapshot',
    metadata: { type: 'file-history-snapshot' },
  },
  {
    type: 'error',
    content: 'Rate limit exceeded. Retry in 5s.',
    ts: NOW_SECS - 195,
    category: 'error',
    metadata: { retryInMs: 5000, retryAttempt: 1 },
  },
  { type: 'user', content: 'Run the tests', ts: NOW_SECS - 190, pending: true },
  {
    type: 'assistant',
    content:
      "All tests pass. Here's the summary:\n\n| Module | Lines |\n|--------|-------|\n| middleware.rs | 120 |\n| validator.rs | 180 |",
    ts: NOW_SECS - 180,
  },
]

// ── Meta ─────────────────────────────────────────────────────────────────────

const meta = {
  title: 'Gallery',
  component: BlockGallery,
  decorators: [withChatContext],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof BlockGallery>

export default meta
type Story = StoryObj<typeof meta>

/** 1. Chat mode — clean bubble UI for end users. */
export const Chat: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry, title: 'Chat Mode' },
}

/** 2. Developer Rich — ALL rich detail cards + ActionLog (all action categories). */
export const DeveloperRich: Story = {
  args: {
    blocks: allBlocks,
    renderers: developerRegistry,
    title: 'Developer — Rich',
    filterBar: true,
    actionLogMessages: richPaneMessages,
  },
}

/** 2b. Developer Rich + JSON mode on — all cards show raw JSON by default. */
export const DeveloperRichWithRawJson: Story = {
  args: {
    blocks: [...allBlocks, ...devEnrichedBlocks],
    renderers: developerRegistry,
    title: 'Developer — JSON Mode',
    filterBar: true,
    defaultJsonMode: true,
    actionLogMessages: richPaneMessages,
  },
}

/** Compact — used in Live Monitor side panel. */
export const Compact: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry, compact: true, title: 'Compact' },
}

/** 4. RichPane — Live Monitor terminal view (all RichMessage types). */
export const RichPaneView: Story = {
  args: { blocks: [], renderers: {}, title: 'RichPane — Live Monitor' },
  render: () => (
    <div className="max-w-4xl mx-auto p-6">
      <div className="flex items-center gap-4 text-xs text-gray-400 dark:text-gray-500 border-b border-gray-200 dark:border-gray-700 pb-3 mb-6">
        <span className="font-bold text-sm text-gray-700 dark:text-gray-300">
          RichPane — Live Monitor Terminal
        </span>
        <span>{richPaneMessages.length} messages</span>
        <span>All RichMessage types</span>
      </div>
      <div style={{ height: 600 }}>
        <RichPane
          messages={richPaneMessages}
          isVisible={true}
          verboseMode={true}
          bufferDone={true}
        />
      </div>
    </div>
  ),
}
