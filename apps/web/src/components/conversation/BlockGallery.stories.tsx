/**
 * Gallery — THE single source of truth for all conversation UI.
 * Renders ALL 7 block types × ALL variants in one scrollable view.
 *
 * 3 display modes (matching the spec):
 *   1. Chat       — clean bubble UI for end users
 *   2. Developer  — rich detail cards (merged from RichPane pipeline)
 *   3. JSON       — raw JSON dump of every block
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

// ── JSON renderer — raw block dump ──────────────────────────────────────────

function JsonBlockRenderer({ block }: { block: ConversationBlock }) {
  return (
    <pre className="text-[11px] font-mono bg-gray-900 text-green-400 rounded p-3 overflow-x-auto max-h-64 overflow-y-auto whitespace-pre-wrap">
      {JSON.stringify(
        block,
        (_key, value) => (typeof value === 'bigint' ? Number(value) : value),
        2,
      )}
    </pre>
  )
}

const jsonRegistry: BlockRenderers = {
  user: JsonBlockRenderer,
  assistant: JsonBlockRenderer,
  interaction: JsonBlockRenderer,
  turn_boundary: JsonBlockRenderer,
  notice: JsonBlockRenderer,
  system: JsonBlockRenderer,
  progress: JsonBlockRenderer,
}

// ── Gallery layout ──────────────────────────────────────────────────────────

function BlockGallery({
  blocks,
  renderers,
  compact,
  title,
  filterBar,
}: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
  compact?: boolean
  title?: string
  filterBar?: boolean
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
          <ConversationThread
            blocks={group.blocks}
            renderers={renderers}
            compact={compact}
            filterBar={filterBar}
          />
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

/** 2. Developer Rich — ALL rich detail cards with CategoryFilterBar. */
export const DeveloperRich: Story = {
  args: {
    blocks: allBlocks,
    renderers: developerRegistry,
    title: 'Developer — Rich',
    filterBar: true,
  },
}

/** 2b. Developer Rich + rawJson — extra detail panels visible. */
export const DeveloperRichWithRawJson: Story = {
  args: {
    blocks: [...allBlocks, ...devEnrichedBlocks],
    renderers: developerRegistry,
    title: 'Developer — Rich + rawJson',
    filterBar: true,
  },
}

/** 3. Developer JSON — raw JSON dump of every block. */
export const DeveloperJSON: Story = {
  args: { blocks: allBlocks, renderers: jsonRegistry, title: 'Developer — JSON' },
}

/** Compact — used in Live Monitor side panel. */
export const Compact: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry, compact: true, title: 'Compact' },
}
