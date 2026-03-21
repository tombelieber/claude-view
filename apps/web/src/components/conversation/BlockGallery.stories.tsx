import type { ConversationBlock } from '@claude-view/shared/types/blocks'
/**
 * Gallery stories — render ALL block types × ALL variants in a single scrollable view.
 * One story per rendering mode (Chat, Developer, Compact).
 */
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
import { devSystemBlocks } from '../../stories/fixtures-developer'
import { ConversationThread } from './ConversationThread'
import { chatRegistry } from './blocks/chat/registry'
import { developerRegistry } from './blocks/developer/registry'

// ── Every single variant, grouped by block type ─────────────────────────────

const allBlocks: ConversationBlock[] = [
  // ── UserBlock (6 variants) ──
  userBlocks.normal,
  userBlocks.sent,
  userBlocks.optimistic,
  userBlocks.sending,
  userBlocks.failed,
  userBlocks.long,

  // ── AssistantBlock (7 variants) ──
  assistantBlocks.textOnly,
  assistantBlocks.streaming,
  assistantBlocks.withThinking,
  assistantBlocks.withTools,
  assistantBlocks.withRunningTool,
  assistantBlocks.withToolError,
  assistantBlocks.markdown,

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

  // ── SystemBlock (all variants) ──
  systemBlocks.taskStarted,
  systemBlocks.taskProgress,
  systemBlocks.taskCompleted,
  systemBlocks.taskFailed,
  devSystemBlocks.sessionInit,
  devSystemBlocks.sessionStatus,
  devSystemBlocks.elicitationComplete,
  devSystemBlocks.hookEvent,
  devSystemBlocks.hookEventError,
  devSystemBlocks.filesSaved,
  devSystemBlocks.commandOutput,
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

// ── Gallery component ───────────────────────────────────────────────────────

function BlockGallery({
  blocks,
  renderers,
  compact,
}: {
  blocks: ConversationBlock[]
  renderers: Parameters<typeof ConversationThread>[0]['renderers']
  compact?: boolean
}) {
  // Group blocks by type for section headers
  const groups: { type: string; blocks: ConversationBlock[] }[] = []
  let current: { type: string; blocks: ConversationBlock[] } | null = null

  for (const block of blocks) {
    if (!current || current.type !== block.type) {
      current = { type: block.type, blocks: [] }
      groups.push(current)
    }
    current.blocks.push(block)
  }

  return (
    <div className="max-w-4xl mx-auto p-6 space-y-8">
      {groups.map((group) => (
        <section key={group.type}>
          <h2 className="text-sm font-bold uppercase tracking-wider text-gray-400 dark:text-gray-500 mb-3 border-b border-gray-200 dark:border-gray-700 pb-1">
            {group.type} ({group.blocks.length} variants)
          </h2>
          <ConversationThread blocks={group.blocks} renderers={renderers} compact={compact} />
        </section>
      ))}
    </div>
  )
}

// ── Meta ─────────────────────────────────────────────────────────────────────

const meta = {
  title: 'Gallery/ConversationBlocks',
  component: BlockGallery,
  decorators: [withChatContext],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof BlockGallery>

export default meta
type Story = StoryObj<typeof meta>

/** Chat mode — ALL 7 block types × ALL variants (49 blocks total). */
export const ChatMode: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry },
}

/** Developer mode — ALL 7 block types × ALL variants (49 blocks total). */
export const DeveloperMode: Story = {
  args: { blocks: allBlocks, renderers: developerRegistry },
}

/** Compact mode — ALL variants in compact layout. */
export const CompactMode: Story = {
  args: { blocks: allBlocks, renderers: chatRegistry, compact: true },
}
