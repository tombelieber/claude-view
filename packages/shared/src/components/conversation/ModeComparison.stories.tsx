/**
 * Mode Comparison Gallery — side-by-side rendering of the SAME blocks across display modes.
 *
 * Shows how each ConversationBlock renders in three display modes:
 *   1. Chat         — clean bubble UI (chatRegistry, sky theme)
 *   2. Developer    — rich detail cards (developerRegistry, indigo theme)
 *   3. Developer JSON — raw JSON tree (developerRegistry + globalJsonMode, amber theme)
 *
 * Covers ALL block types × representative variants so mode differences
 * can be compared one-by-one.
 */
import type { ConversationBlock, ProgressBlock } from '../../types/blocks'
import type { Meta, StoryObj } from '@storybook/react-vite'
import type { ComponentType } from 'react'
import { withConversationActions } from '../../stories/decorators'
import {
  agentGroupBlocks,
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
import { ChatAgentGroupRow } from './blocks/chat/AgentGroupRow'
import { chatRegistry } from './blocks/chat/registry'
import { DevAgentGroupRow } from './blocks/developer/AgentGroupRow'
import { DefaultExpandedProvider } from './blocks/developer/default-expanded-context'
import { JsonModeProvider } from './blocks/developer/json-mode-context'
import { developerRegistry } from './blocks/developer/registry'
import type { BlockRenderers } from './types'

// ── Comparison definitions ──────────────────────────────────────────────────

interface ModeComparison {
  label: string
  description: string
  blocks: ConversationBlock[]
  /** Optional custom renderer per column — used for agent groups that aren't single blocks. */
  customRender?: {
    chat?: React.ReactNode
    developer?: React.ReactNode
    json?: React.ReactNode
  }
}

const comparisons: ModeComparison[] = [
  // ── 1. User ──
  {
    label: 'user',
    description: 'Chat bubble vs DevUserBlock (status dot + ID) vs raw JSON tree',
    blocks: [userBlocks.normal],
  },

  // ── 2. User (all variants) ──
  {
    label: 'user: all variants',
    description: 'sent / optimistic / sending / failed / long — status rendering differences',
    blocks: [
      userBlocks.sent,
      userBlocks.optimistic,
      userBlocks.sending,
      userBlocks.failed,
      userBlocks.long,
    ],
  },

  // ── 2b. User (new: image, sidechain, agent) ──
  {
    label: 'user: image + sidechain + agent',
    description: 'New fields: image content, sidechain branching, agent attribution',
    blocks: [userBlocks.withImage, userBlocks.sidechain, userBlocks.fromAgent],
  },

  // ── 3. Assistant (text only) ──
  {
    label: 'assistant (text)',
    description: 'Chat markdown bubble vs DevAssistantBlock segments vs raw JSON',
    blocks: [assistantBlocks.textOnly],
  },

  // ── 4. Assistant (thinking) ──
  {
    label: 'assistant (thinking)',
    description: 'Chat hides thinking vs Dev shows collapsible thinking block vs JSON tree',
    blocks: [assistantBlocks.withThinking],
  },

  // ── 5. Assistant (tools) ──
  {
    label: 'assistant (tools)',
    description:
      'Chat compact tool pill vs Dev ToolCard (expandable) vs JSON with full input/output',
    blocks: [assistantBlocks.withTools],
  },

  // ── 6. Assistant (all tool variants) ──
  {
    label: 'assistant (all tool variants)',
    description: 'Diff, JSON, MCP, agent tools — Dev shows rich tool-specific cards',
    blocks: [assistantBlocks.withAllToolVariants as ConversationBlock],
  },

  // ── 7. Assistant (streaming) ──
  {
    label: 'assistant (streaming + running tool)',
    description: 'Chat shows cursor vs Dev shows streaming indicator + running tool spinner',
    blocks: [
      assistantBlocks.streaming as ConversationBlock,
      assistantBlocks.withRunningTool as ConversationBlock,
    ],
  },

  // ── 8. Assistant (markdown) ──
  {
    label: 'assistant (markdown)',
    description: 'Chat renders markdown (tables, code, lists) vs Dev segments vs JSON raw',
    blocks: [assistantBlocks.markdown as ConversationBlock],
  },

  // ── 8b. Assistant (sidechain + agent) ──
  {
    label: 'assistant: sidechain + agent',
    description: 'New fields: sidechain reply, agent-attributed message',
    blocks: [
      assistantBlocks.sidechainReply as ConversationBlock,
      assistantBlocks.fromAgent as ConversationBlock,
    ],
  },

  // ── 9. Interaction: permission (pending) ──
  {
    label: 'interaction: permission (pending)',
    description: 'Chat PermissionCard vs Dev PermissionCard (with tool metadata) vs JSON',
    blocks: [interactionBlocks.permissionPending as ConversationBlock],
  },

  // ── 10. Interaction: permission (resolved) ──
  {
    label: 'interaction: permission (resolved)',
    description: 'Chat shows resolved state vs Dev shows resolution + timing vs JSON',
    blocks: [interactionBlocks.permissionResolved as ConversationBlock],
  },

  // ── 11. Interaction: question ──
  {
    label: 'interaction: question',
    description: 'Chat AskUserQuestionCard vs Dev AskUserQuestionCard (with options) vs JSON',
    blocks: [interactionBlocks.questionPending as ConversationBlock],
  },

  // ── 12. Interaction: plan ──
  {
    label: 'interaction: plan',
    description: 'Chat PlanApprovalCard vs Dev PlanApprovalCard (numbered steps) vs JSON',
    blocks: [interactionBlocks.planPending as ConversationBlock],
  },

  // ── 13. Interaction: elicitation ──
  {
    label: 'interaction: elicitation',
    description: 'Chat ElicitationCard vs Dev ElicitationCard vs JSON raw schema',
    blocks: [interactionBlocks.elicitationPending as ConversationBlock],
  },

  // ── 14. Turn boundary: success ──
  {
    label: 'turn_boundary: success',
    description: 'Chat thin divider vs Dev cost/token summary bar vs JSON raw',
    blocks: [turnBoundaryBlocks.success as ConversationBlock],
  },

  // ── 15. Turn boundary (all variants) ──
  {
    label: 'turn_boundary: all variants',
    description: 'success / cheap / free / error / max_turns — how modes show cost & errors',
    blocks: [
      turnBoundaryBlocks.success as ConversationBlock,
      turnBoundaryBlocks.cheap as ConversationBlock,
      turnBoundaryBlocks.free as ConversationBlock,
      turnBoundaryBlocks.error as ConversationBlock,
      turnBoundaryBlocks.maxTurns as ConversationBlock,
    ],
  },

  // ── 15b. Turn boundary (hook details) ──
  {
    label: 'turn_boundary: hook errors + prevented',
    description: 'New fields: hookInfos, hookErrors, hookCount, preventedContinuation',
    blocks: [
      turnBoundaryBlocks.withHookErrors as ConversationBlock,
      turnBoundaryBlocks.preventedContinuation as ConversationBlock,
    ],
  },

  // ── 16. Notice: errors ──
  {
    label: 'notice: errors',
    description: 'Chat error toast vs Dev error card (AlertCircle + variant) vs JSON',
    blocks: [
      noticeBlocks.assistantError as ConversationBlock,
      noticeBlocks.billingError as ConversationBlock,
      noticeBlocks.serverError as ConversationBlock,
      noticeBlocks.error as ConversationBlock,
      noticeBlocks.fatalError as ConversationBlock,
    ],
  },

  // ── 17. Notice: rate limit ──
  {
    label: 'notice: rate_limit',
    description: 'Chat warning banner vs Dev rate-limit card (warning + rejected) vs JSON',
    blocks: [
      noticeBlocks.rateLimitWarning as ConversationBlock,
      noticeBlocks.rateLimitRejected as ConversationBlock,
      noticeBlocks.rateLimitWithRetry as ConversationBlock,
    ],
  },

  // ── 18. Notice: auth ──
  {
    label: 'notice: auth',
    description: 'Chat auth status vs Dev auth card (authenticating + error) vs JSON',
    blocks: [
      noticeBlocks.authenticating as ConversationBlock,
      noticeBlocks.authError as ConversationBlock,
    ],
  },

  // ── 19. Notice: session lifecycle ──
  {
    label: 'notice: session lifecycle',
    description: 'Chat session banner vs Dev session card (closed + resumed) vs JSON',
    blocks: [
      noticeBlocks.sessionClosed as ConversationBlock,
      noticeBlocks.sessionResumed as ConversationBlock,
    ],
  },

  // ── 20. Notice: context compacted ──
  {
    label: 'notice: context_compacted',
    description: 'Chat compact notice vs Dev compact card (auto + manual) vs JSON',
    blocks: [
      noticeBlocks.contextCompacted as ConversationBlock,
      noticeBlocks.contextCompactedManual as ConversationBlock,
    ],
  },

  // ── 21. Notice: prompt suggestion ──
  {
    label: 'notice: prompt_suggestion',
    description: 'Chat clickable pill vs Dev prompt card vs JSON',
    blocks: [noticeBlocks.promptSuggestion as ConversationBlock],
  },

  // ── 22. System: session_init ──
  {
    label: 'system: session_init',
    description: 'Chat hides system vs Dev shows session init (model, tools, cwd) vs JSON',
    blocks: [devSystemBlocks.sessionInit as ConversationBlock],
  },

  // ── 23. System: session_status ──
  {
    label: 'system: session_status',
    description: 'Chat hides vs Dev shows session status (compacting + idle) vs JSON',
    blocks: [
      devSystemBlocks.sessionStatus as ConversationBlock,
      devSystemBlocks.sessionStatusIdle as ConversationBlock,
    ],
  },

  // ── 24. System: hook events ──
  {
    label: 'system: hook_event',
    description: 'Chat hides vs Dev hook event card (success + error) vs JSON',
    blocks: [
      devSystemBlocks.hookEvent as ConversationBlock,
      devSystemBlocks.hookEventError as ConversationBlock,
    ],
  },

  // ── 25. System: task lifecycle ──
  {
    label: 'system: task lifecycle',
    description: 'Chat hides vs Dev task cards (started → progress → completed → failed) vs JSON',
    blocks: [
      systemBlocks.taskStarted as ConversationBlock,
      systemBlocks.taskProgress as ConversationBlock,
      systemBlocks.taskCompleted as ConversationBlock,
      systemBlocks.taskFailed as ConversationBlock,
    ],
  },

  // ── 26. System: file operations ──
  {
    label: 'system: files_saved',
    description: 'Chat hides vs Dev file save card (success + with failures) vs JSON',
    blocks: [
      devSystemBlocks.filesSaved as ConversationBlock,
      devSystemBlocks.filesSavedWithFailures as ConversationBlock,
    ],
  },

  // ── 27. System: output events ──
  {
    label: 'system: output events',
    description: 'Chat hides vs Dev output cards (command_output + stream_delta + unknown) vs JSON',
    blocks: [
      devSystemBlocks.commandOutput as ConversationBlock,
      devSystemBlocks.streamDelta as ConversationBlock,
      devSystemBlocks.unknown as ConversationBlock,
    ],
  },

  // ── 28. System: metadata ──
  {
    label: 'system: metadata',
    description:
      'Chat hides vs Dev metadata cards (ai_title + last_prompt + informational) vs JSON',
    blocks: [
      devSystemBlocks.aiTitle as ConversationBlock,
      devSystemBlocks.lastPrompt as ConversationBlock,
      devSystemBlocks.informational as ConversationBlock,
    ],
  },

  // ── 29. System: queue + file_history ──
  {
    label: 'system: queue + file_history',
    description: 'Chat hides vs Dev queue/snapshot cards vs JSON',
    blocks: [
      devSystemBlocks.queueOperation as ConversationBlock,
      devSystemBlocks.fileHistorySnapshot as ConversationBlock,
      devSystemBlocks.localCommand as ConversationBlock,
    ],
  },

  // ── 30. System: elicitation_complete ──
  {
    label: 'system: elicitation_complete',
    description: 'Chat hides vs Dev elicitation complete card (MCP server + ID) vs JSON',
    blocks: [devSystemBlocks.elicitationComplete as ConversationBlock],
  },

  // ── 30b. System: new variants (pr_link, custom_title, plan_content) ──
  {
    label: 'system: new variants',
    description: 'PR link, custom title, plan content — newly parsed from JSONL',
    blocks: [
      systemBlocks.prLink as ConversationBlock,
      systemBlocks.customTitle as ConversationBlock,
      systemBlocks.planContent as ConversationBlock,
    ],
  },

  // ── 31. Progress: all variants ──
  {
    label: 'progress: all variants',
    description:
      'Chat progress indicator vs Dev progress cards (bash/agent/hook/mcp/queue/search/query) vs JSON',
    blocks: [
      progressBlocks.bash as ConversationBlock,
      progressBlocks.agent as ConversationBlock,
      progressBlocks.hook as ConversationBlock,
      progressBlocks.mcp as ConversationBlock,
      progressBlocks.taskQueue as ConversationBlock,
      progressBlocks.search as ConversationBlock,
      progressBlocks.query as ConversationBlock,
    ],
  },

  // ── 31b. Agent Group (collapsed) ──
  {
    label: 'agent group',
    description:
      'Agent progress group: Chat collapsed by default, Developer expanded. Shows description + tool summary (Glob ×1, Grep ×1, Read ×3, Bash ×1).',
    blocks: agentGroupBlocks as ConversationBlock[],
    customRender: {
      chat: <ChatAgentGroupRow blocks={agentGroupBlocks as ProgressBlock[]} />,
      developer: <DevAgentGroupRow blocks={agentGroupBlocks as ProgressBlock[]} />,
    },
  },

  // ── 31. Developer-enriched: rawJson extras ──
  {
    label: 'developer-enriched: rawJson + imagePastes',
    description:
      'Chat simple view vs Dev enriched blocks (rawJson, imagePastes, permissionMode) vs JSON full tree',
    blocks: [
      devUserBlocks.withRawJson as ConversationBlock,
      devUserBlocks.withImagePastes as ConversationBlock,
      devAssistantBlocks.withRawJson as ConversationBlock,
      devAssistantBlocks.withPermissionMode as ConversationBlock,
    ],
  },

  // ── 32. Developer-enriched: system with rawJson ──
  {
    label: 'developer-enriched: system rawJson',
    description: 'Chat hides vs Dev system enriched (retry, apiError, hooks, all) vs JSON',
    blocks: [
      devSystemBlocksWithRawJson.withRetry as ConversationBlock,
      devSystemBlocksWithRawJson.withApiError as ConversationBlock,
      devSystemBlocksWithRawJson.withHooks as ConversationBlock,
      devSystemBlocksWithRawJson.withHookErrors as ConversationBlock,
      devSystemBlocksWithRawJson.withAll as ConversationBlock,
    ],
  },

  // ── 33. Developer-enriched: turn boundary ──
  {
    label: 'developer-enriched: turn_boundary',
    description: 'Chat divider vs Dev turn boundary with permission denials vs JSON',
    blocks: [devTurnBoundaryBlocks.withPermissionDenials as ConversationBlock],
  },
]

// ── Block list (no Virtuoso — full height) ──────────────────────────────────

function BlockList({
  blocks,
  renderers,
}: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
}) {
  return (
    <div className="space-y-1.5">
      {blocks.map((block) => {
        const Renderer = renderers[block.type] as
          | ComponentType<{ block: ConversationBlock }>
          | undefined
        if (!Renderer) return null
        if (renderers.canRender && !renderers.canRender(block)) return null
        return (
          <div key={block.id}>
            <Renderer block={block} />
          </div>
        )
      })}
    </div>
  )
}

// ── Comparison Row ──────────────────────────────────────────────────────────

function ModeComparisonRow({ label, description, blocks, customRender }: ModeComparison) {
  return (
    <section className="border border-gray-700 rounded-lg overflow-hidden">
      {/* Row header */}
      <div className="bg-gray-800 px-4 py-2 border-b border-gray-700">
        <h2 className="text-sm font-bold text-white font-mono">{label}</h2>
        <p className="text-xs text-gray-400 mt-0.5">{description}</p>
      </div>

      {/* Three-column comparison — grid stretches all columns to tallest */}
      <div className="grid grid-cols-3 divide-x divide-gray-700 items-start">
        {/* Chat mode */}
        <div className="p-3">
          <div className="text-xs font-bold uppercase tracking-wider text-sky-400 mb-2">
            Chat Mode
          </div>
          {customRender?.chat ?? <BlockList blocks={blocks} renderers={chatRegistry} />}
        </div>

        {/* Developer Rich (expanded) */}
        <div className="p-3">
          <div className="text-xs font-bold uppercase tracking-wider text-indigo-400 mb-2">
            Developer — Rich
          </div>
          <DefaultExpandedProvider value={true}>
            <JsonModeProvider value={false}>
              {customRender?.developer ?? (
                <BlockList blocks={blocks} renderers={developerRegistry} />
              )}
            </JsonModeProvider>
          </DefaultExpandedProvider>
        </div>

        {/* Developer JSON (same as Rich but JSON toggle = on) */}
        <div className="p-3">
          <div className="text-xs font-bold uppercase tracking-wider text-amber-400 mb-2">
            Developer — JSON
          </div>
          <DefaultExpandedProvider value={true}>
            <JsonModeProvider value={true}>
              {customRender?.json ?? <BlockList blocks={blocks} renderers={developerRegistry} />}
            </JsonModeProvider>
          </DefaultExpandedProvider>
        </div>
      </div>
    </section>
  )
}

// ── Gallery ─────────────────────────────────────────────────────────────────

function ModeComparisonGallery(_props: {
  blocks: ConversationBlock[]
  renderers: BlockRenderers
}) {
  return (
    <div className="w-full px-4 py-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-4 text-xs text-gray-400 border-b border-gray-700 pb-3">
        <span className="font-bold text-sm text-white">
          Mode Comparison — Chat vs Developer (Rich) vs Developer (JSON)
        </span>
        <span>{comparisons.length} comparison rows</span>
        <span className="text-sky-400">Chat</span>
        <span className="text-indigo-400">Developer Rich</span>
        <span className="text-amber-400">Developer JSON</span>
      </div>

      {comparisons.map((comp) => (
        <ModeComparisonRow key={comp.label} {...comp} />
      ))}
    </div>
  )
}

// ── Meta ─────────────────────────────────────────────────────────────────────

const meta = {
  title: 'Gallery/Comparison',
  component: ModeComparisonGallery,
  decorators: [withConversationActions],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ModeComparisonGallery>

export default meta
type Story = StoryObj<typeof meta>

/** Side-by-side comparison of ALL block types across Chat / Developer Rich / Developer JSON modes. */
export const ModeComparison: Story = {
  args: { blocks: [], renderers: developerRegistry },
}
