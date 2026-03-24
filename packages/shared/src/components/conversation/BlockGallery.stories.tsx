/**
 * Gallery — THE single source of truth for all conversation UI.
 * Renders ALL 7 block types x ALL variants in one scrollable view.
 *
 * Display modes:
 *   1. Chat       — clean bubble UI for end users
 *   2. Developer  — rich detail cards (use { } toggle for JSON mode)
 */
import type { ConversationBlock } from '../../types/blocks'
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
import { ConversationThread } from './ConversationThread'
import { chatRegistry } from './blocks/chat/registry'
import { developerRegistry } from './blocks/developer/registry'
import type { BlockRenderers } from './types'

// ── Every single variant, grouped by block type ─────────────────────────────

const allBlocks: ConversationBlock[] = [
  // ── UserBlock (9 variants) ──
  userBlocks.normal,
  userBlocks.sent,
  userBlocks.optimistic,
  userBlocks.sending,
  userBlocks.failed,
  userBlocks.long,
  userBlocks.withImage,
  userBlocks.sidechain,
  userBlocks.fromAgent,

  // ── AssistantBlock (10 variants) ──
  assistantBlocks.textOnly,
  assistantBlocks.streaming,
  assistantBlocks.withThinking,
  assistantBlocks.withTools,
  assistantBlocks.withRunningTool,
  assistantBlocks.withToolError,
  assistantBlocks.markdown,
  assistantBlocks.withAllToolVariants,
  assistantBlocks.sidechainReply,
  assistantBlocks.fromAgent,

  // ── InteractionBlock (5 variants) ──
  interactionBlocks.permissionPending,
  interactionBlocks.permissionResolved,
  interactionBlocks.questionPending,
  interactionBlocks.planPending,
  interactionBlocks.elicitationPending,

  // ── TurnBoundaryBlock (7 variants) ──
  turnBoundaryBlocks.success,
  turnBoundaryBlocks.cheap,
  turnBoundaryBlocks.free,
  turnBoundaryBlocks.error,
  turnBoundaryBlocks.maxTurns,
  turnBoundaryBlocks.withHookErrors,
  turnBoundaryBlocks.preventedContinuation,

  // ── NoticeBlock (16 variants) ──
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
  noticeBlocks.rateLimitWithRetry,

  // ── SystemBlock (20 variants) ──
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
  systemBlocks.prLink as ConversationBlock,
  systemBlocks.customTitle as ConversationBlock,
  systemBlocks.planContent as ConversationBlock,

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
}: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
  title?: string
  filterBar?: boolean
  defaultJsonMode?: boolean
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
    </div>
  )
}

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

/** 2. Developer Rich — ALL rich detail cards (all action categories). */
export const DeveloperRich: Story = {
  args: {
    blocks: allBlocks,
    renderers: developerRegistry,
    title: 'Developer — Rich',
    filterBar: true,
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
  },
}

/** Compact — used in Live Monitor side panel. */
export const Compact: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry, compact: true, title: 'Compact' },
}
