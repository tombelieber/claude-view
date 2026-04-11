/**
 * BlockGallery — 3-column comparison of every block type across Chat | Dev | Dev+JSON.
 *
 * This is the master visual regression surface. Every block type with every variant
 * appears here so you can spot rendering differences across view modes at a glance.
 */
import type { Meta, StoryObj } from '@storybook/react-vite'
import type { ReactNode } from 'react'
import { ConversationActionsProvider } from '../../../contexts/conversation-actions-context'
import type { ConversationActions } from '../../../contexts/conversation-actions-context'
import {
  interactionBlocks,
  noticeBlocks,
  progressBlocks,
  systemBlocks,
  teamTranscriptBlocks,
  turnBoundaryBlocks,
} from '../../../stories/fixtures'
import { devSystemBlocks } from '../../../stories/fixtures-developer'
import { ChatInteractionBlock } from './chat/InteractionBlock'
import { ChatNoticeBlock } from './chat/NoticeBlock'
import { ChatProgressBlock } from './chat/ProgressBlock'
import { ChatSystemBlock } from './chat/SystemBlock'
import { ChatTeamTranscriptBlock } from './chat/TeamTranscriptBlock'
import { ChatTurnBoundary } from './chat/TurnBoundary'
import { DefaultExpandedProvider } from './developer/default-expanded-context'
import { DevInteractionBlock } from './developer/InteractionBlock'
import { JsonModeProvider } from './developer/json-mode-context'
import { DevNoticeBlock } from './developer/NoticeBlock'
import { DevProgressBlock } from './developer/ProgressBlock'
import { DevSystemBlock } from './developer/SystemBlock'
import { DevTeamTranscriptBlock } from './developer/TeamTranscriptBlock'
import { DevTurnBoundary } from './developer/TurnBoundary'

// ── Shared helpers ──────────────────────────────────────────────────────────

const noopActions: ConversationActions = {
  retryMessage: () => {},
  stopTask: () => {},
  respondPermission: () => {},
  answerQuestion: () => {},
  approvePlan: () => {},
  submitElicitation: () => {},
}

function GalleryRow({
  label,
  chat,
  dev,
  devJson,
}: {
  label: string
  chat: ReactNode
  dev: ReactNode
  devJson: ReactNode
}) {
  return (
    <div className="mb-6">
      <h3 className="text-sm font-semibold text-gray-600 dark:text-gray-400 mb-2 font-mono">
        {label}
      </h3>
      <div className="grid grid-cols-3 gap-3">
        <div className="min-w-0 overflow-hidden">
          <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 mb-1 uppercase tracking-wider">
            Chat
          </div>
          <div className="rounded-md border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-900">
            {chat}
          </div>
        </div>
        <div className="min-w-0 overflow-hidden">
          <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 mb-1 uppercase tracking-wider">
            Developer
          </div>
          <div className="rounded-md border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-900">
            <JsonModeProvider value={false}>
              <DefaultExpandedProvider value={true}>{dev}</DefaultExpandedProvider>
            </JsonModeProvider>
          </div>
        </div>
        <div className="min-w-0 overflow-hidden">
          <div className="text-[10px] font-mono text-gray-400 dark:text-gray-500 mb-1 uppercase tracking-wider">
            Developer (JSON)
          </div>
          <div className="rounded-md border border-gray-200 dark:border-gray-700 p-2 bg-white dark:bg-gray-900">
            <JsonModeProvider value={true}>
              <DefaultExpandedProvider value={true}>{devJson}</DefaultExpandedProvider>
            </JsonModeProvider>
          </div>
        </div>
      </div>
    </div>
  )
}

/** Wraps the gallery in providers required by all block renderers. */
function GalleryDecorator({ children }: { children: ReactNode }) {
  return (
    <ConversationActionsProvider actions={noopActions}>
      <div className="w-full max-w-[1400px] mx-auto p-4">{children}</div>
    </ConversationActionsProvider>
  )
}

// ── Meta ────────────────────────────────────────────────────────────────────

/** Blank component — each story renders its own layout */
function GalleryStub() {
  return null
}

const meta = {
  title: 'BlockGallery',
  component: GalleryStub,
  parameters: {
    layout: 'fullscreen',
    docs: { canvas: { sourceState: 'hidden' } },
  },
  decorators: [
    (Story) => (
      <GalleryDecorator>
        <Story />
      </GalleryDecorator>
    ),
  ],
} satisfies Meta<typeof GalleryStub>

export default meta
type Story = StoryObj<typeof meta>

// ── System Block Variants ──────────────────────────────────────────────────

function SystemGalleryRow({
  label,
  fixture,
}: { label: string; fixture: typeof systemBlocks.taskStarted }) {
  const devFixture =
    devSystemBlocks[
      (label.charAt(0).toLowerCase() + label.slice(1)) as keyof typeof devSystemBlocks
    ] ?? fixture
  return (
    <GalleryRow
      label={`System / ${label}`}
      chat={<ChatSystemBlock block={fixture} />}
      dev={<DevSystemBlock block={devFixture} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devFixture,
            rawJson: devFixture.rawJson ?? (devFixture.data as Record<string, unknown>),
          }}
        />
      }
    />
  )
}

export const SystemTaskStarted: Story = {
  render: () => <SystemGalleryRow label="TaskStarted" fixture={systemBlocks.taskStarted} />,
}

export const SystemTaskProgress: Story = {
  render: () => <SystemGalleryRow label="TaskProgress" fixture={systemBlocks.taskProgress} />,
}

export const SystemTaskCompleted: Story = {
  render: () => <SystemGalleryRow label="TaskCompleted" fixture={systemBlocks.taskCompleted} />,
}

export const SystemTaskFailed: Story = {
  render: () => <SystemGalleryRow label="TaskFailed" fixture={systemBlocks.taskFailed} />,
}

export const SystemQueueOperation: Story = {
  render: () => <SystemGalleryRow label="QueueOperation" fixture={systemBlocks.queueOperation} />,
}

export const SystemPrLink: Story = {
  render: () => <SystemGalleryRow label="PrLink" fixture={systemBlocks.prLink} />,
}

export const SystemCustomTitle: Story = {
  render: () => <SystemGalleryRow label="CustomTitle" fixture={systemBlocks.customTitle} />,
}

export const SystemPlanContent: Story = {
  render: () => <SystemGalleryRow label="PlanContent" fixture={systemBlocks.planContent} />,
}

export const SystemAgentName: Story = {
  render: () => <SystemGalleryRow label="AgentName" fixture={systemBlocks.agentName} />,
}

export const SystemSessionInit: Story = {
  render: () => (
    <GalleryRow
      label="System / SessionInit"
      chat={<ChatSystemBlock block={systemBlocks.sessionInit} />}
      dev={<DevSystemBlock block={devSystemBlocks.sessionInit} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.sessionInit,
            rawJson: devSystemBlocks.sessionInit.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemSessionStatus: Story = {
  render: () => (
    <GalleryRow
      label="System / SessionStatus"
      chat={<ChatSystemBlock block={systemBlocks.sessionStatus} />}
      dev={<DevSystemBlock block={devSystemBlocks.sessionStatus} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.sessionStatus,
            rawJson: devSystemBlocks.sessionStatus.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemElicitationComplete: Story = {
  render: () => (
    <GalleryRow
      label="System / ElicitationComplete"
      chat={<ChatSystemBlock block={systemBlocks.elicitationComplete} />}
      dev={<DevSystemBlock block={devSystemBlocks.elicitationComplete} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.elicitationComplete,
            rawJson: devSystemBlocks.elicitationComplete.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemHookEvent: Story = {
  render: () => (
    <GalleryRow
      label="System / HookEvent (success)"
      chat={<ChatSystemBlock block={systemBlocks.hookEvent} />}
      dev={<DevSystemBlock block={devSystemBlocks.hookEvent} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.hookEvent,
            rawJson: devSystemBlocks.hookEvent.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemHookEventError: Story = {
  render: () => (
    <GalleryRow
      label="System / HookEvent (error)"
      chat={<ChatSystemBlock block={systemBlocks.hookEventError} />}
      dev={<DevSystemBlock block={devSystemBlocks.hookEventError} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.hookEventError,
            rawJson: devSystemBlocks.hookEventError.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemFilesSaved: Story = {
  render: () => (
    <GalleryRow
      label="System / FilesSaved"
      chat={<ChatSystemBlock block={systemBlocks.filesSaved} />}
      dev={<DevSystemBlock block={devSystemBlocks.filesSaved} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.filesSaved,
            rawJson: devSystemBlocks.filesSaved.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemCommandOutput: Story = {
  render: () => (
    <GalleryRow
      label="System / CommandOutput"
      chat={<ChatSystemBlock block={systemBlocks.commandOutput} />}
      dev={<DevSystemBlock block={devSystemBlocks.commandOutput} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.commandOutput,
            rawJson: devSystemBlocks.commandOutput.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemStreamDelta: Story = {
  render: () => (
    <GalleryRow
      label="System / StreamDelta"
      chat={<ChatSystemBlock block={systemBlocks.streamDelta} />}
      dev={<DevSystemBlock block={devSystemBlocks.streamDelta} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.streamDelta,
            rawJson: devSystemBlocks.streamDelta.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemLocalCommand: Story = {
  render: () => (
    <GalleryRow
      label="System / LocalCommand"
      chat={<ChatSystemBlock block={systemBlocks.localCommand} />}
      dev={<DevSystemBlock block={devSystemBlocks.localCommand} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.localCommand,
            rawJson: devSystemBlocks.localCommand.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemFileHistorySnapshot: Story = {
  render: () => (
    <GalleryRow
      label="System / FileHistorySnapshot"
      chat={<ChatSystemBlock block={systemBlocks.fileHistorySnapshot} />}
      dev={<DevSystemBlock block={devSystemBlocks.fileHistorySnapshot} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.fileHistorySnapshot,
            rawJson: devSystemBlocks.fileHistorySnapshot.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemAiTitle: Story = {
  render: () => (
    <GalleryRow
      label="System / AiTitle"
      chat={<ChatSystemBlock block={systemBlocks.aiTitle} />}
      dev={<DevSystemBlock block={devSystemBlocks.aiTitle} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.aiTitle,
            rawJson: devSystemBlocks.aiTitle.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemLastPrompt: Story = {
  render: () => (
    <GalleryRow
      label="System / LastPrompt"
      chat={<ChatSystemBlock block={systemBlocks.lastPrompt} />}
      dev={<DevSystemBlock block={devSystemBlocks.lastPrompt} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.lastPrompt,
            rawJson: devSystemBlocks.lastPrompt.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemWorktreeState: Story = {
  render: () => (
    <GalleryRow
      label="System / WorktreeState"
      chat={<ChatSystemBlock block={systemBlocks.worktreeState} />}
      dev={<DevSystemBlock block={devSystemBlocks.worktreeState} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.worktreeState,
            rawJson: devSystemBlocks.worktreeState.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemInformational: Story = {
  render: () => (
    <GalleryRow
      label="System / Informational"
      chat={<ChatSystemBlock block={systemBlocks.informational} />}
      dev={<DevSystemBlock block={devSystemBlocks.informational} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.informational,
            rawJson: devSystemBlocks.informational.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemAttachmentHook: Story = {
  render: () => (
    <GalleryRow
      label="System / Attachment (async_hook)"
      chat={<ChatSystemBlock block={systemBlocks.attachmentHook} />}
      dev={<DevSystemBlock block={devSystemBlocks.attachment} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.attachment,
            rawJson: devSystemBlocks.attachment.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemAttachmentFile: Story = {
  render: () => (
    <GalleryRow
      label="System / Attachment (file)"
      chat={<ChatSystemBlock block={systemBlocks.attachmentFile} />}
      dev={<DevSystemBlock block={devSystemBlocks.attachment} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.attachment,
            rawJson: devSystemBlocks.attachment.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemPermissionModeChange: Story = {
  render: () => (
    <GalleryRow
      label="System / PermissionModeChange"
      chat={<ChatSystemBlock block={systemBlocks.permissionModeChange} />}
      dev={<DevSystemBlock block={devSystemBlocks.permissionModeChange} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.permissionModeChange,
            rawJson: devSystemBlocks.permissionModeChange.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemScheduledTaskFire: Story = {
  render: () => (
    <GalleryRow
      label="System / ScheduledTaskFire"
      chat={<ChatSystemBlock block={systemBlocks.scheduledTaskFire} />}
      dev={<DevSystemBlock block={devSystemBlocks.scheduledTaskFire} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.scheduledTaskFire,
            rawJson: devSystemBlocks.scheduledTaskFire.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

export const SystemUnknown: Story = {
  render: () => (
    <GalleryRow
      label="System / Unknown"
      chat={<ChatSystemBlock block={systemBlocks.unknownVariant} />}
      dev={<DevSystemBlock block={devSystemBlocks.unknown} />}
      devJson={
        <DevSystemBlock
          block={{
            ...devSystemBlocks.unknown,
            rawJson: devSystemBlocks.unknown.data as Record<string, unknown>,
          }}
        />
      }
    />
  ),
}

// ── Team Transcript ────────────────────────────────────────────────────────

export const TeamTranscriptMultiSpeaker: Story = {
  render: () => (
    <GalleryRow
      label="TeamTranscript / MultiSpeaker"
      chat={<ChatTeamTranscriptBlock block={teamTranscriptBlocks.multiSpeaker} />}
      dev={<DevTeamTranscriptBlock block={teamTranscriptBlocks.multiSpeaker} />}
      devJson={<DevTeamTranscriptBlock block={teamTranscriptBlocks.multiSpeaker} />}
    />
  ),
}

// ── Turn Boundary ──────────────────────────────────────────────────────────

export const TurnBoundarySuccess: Story = {
  render: () => (
    <GalleryRow
      label="TurnBoundary / Success"
      chat={<ChatTurnBoundary block={turnBoundaryBlocks.success} />}
      dev={<DevTurnBoundary block={turnBoundaryBlocks.success} />}
      devJson={<DevTurnBoundary block={turnBoundaryBlocks.success} />}
    />
  ),
}

export const TurnBoundaryError: Story = {
  render: () => (
    <GalleryRow
      label="TurnBoundary / Error"
      chat={<ChatTurnBoundary block={turnBoundaryBlocks.error} />}
      dev={<DevTurnBoundary block={turnBoundaryBlocks.error} />}
      devJson={<DevTurnBoundary block={turnBoundaryBlocks.error} />}
    />
  ),
}

export const TurnBoundaryFullFields: Story = {
  render: () => (
    <GalleryRow
      label="TurnBoundary / Full Fields (all optional fields)"
      chat={<ChatTurnBoundary block={turnBoundaryBlocks.fullFields} />}
      dev={<DevTurnBoundary block={turnBoundaryBlocks.fullFields} />}
      devJson={<DevTurnBoundary block={turnBoundaryBlocks.fullFields} />}
    />
  ),
}

export const TurnBoundaryMaxTurns: Story = {
  render: () => (
    <GalleryRow
      label="TurnBoundary / MaxTurns"
      chat={<ChatTurnBoundary block={turnBoundaryBlocks.maxTurns} />}
      dev={<DevTurnBoundary block={turnBoundaryBlocks.maxTurns} />}
      devJson={<DevTurnBoundary block={turnBoundaryBlocks.maxTurns} />}
    />
  ),
}

export const TurnBoundaryWithHookErrors: Story = {
  render: () => (
    <GalleryRow
      label="TurnBoundary / With Hook Errors"
      chat={<ChatTurnBoundary block={turnBoundaryBlocks.withHookErrors} />}
      dev={<DevTurnBoundary block={turnBoundaryBlocks.withHookErrors} />}
      devJson={<DevTurnBoundary block={turnBoundaryBlocks.withHookErrors} />}
    />
  ),
}

// ── Notice Blocks ──────────────────────────────────────────────────────────

export const NoticeRateLimit: Story = {
  render: () => (
    <GalleryRow
      label="Notice / RateLimit (warning)"
      chat={<ChatNoticeBlock block={noticeBlocks.rateLimitWarning} />}
      dev={<DevNoticeBlock block={noticeBlocks.rateLimitWarning} />}
      devJson={<DevNoticeBlock block={noticeBlocks.rateLimitWarning} />}
    />
  ),
}

export const NoticeRateLimitRejected: Story = {
  render: () => (
    <GalleryRow
      label="Notice / RateLimit (rejected)"
      chat={<ChatNoticeBlock block={noticeBlocks.rateLimitRejected} />}
      dev={<DevNoticeBlock block={noticeBlocks.rateLimitRejected} />}
      devJson={<DevNoticeBlock block={noticeBlocks.rateLimitRejected} />}
    />
  ),
}

export const NoticeContextCompacted: Story = {
  render: () => (
    <GalleryRow
      label="Notice / ContextCompacted"
      chat={<ChatNoticeBlock block={noticeBlocks.contextCompacted} />}
      dev={<DevNoticeBlock block={noticeBlocks.contextCompacted} />}
      devJson={<DevNoticeBlock block={noticeBlocks.contextCompacted} />}
    />
  ),
}

export const NoticeErrorFatal: Story = {
  render: () => (
    <GalleryRow
      label="Notice / Error (fatal)"
      chat={<ChatNoticeBlock block={noticeBlocks.fatalError} />}
      dev={<DevNoticeBlock block={noticeBlocks.fatalError} />}
      devJson={<DevNoticeBlock block={noticeBlocks.fatalError} />}
    />
  ),
}

export const NoticeSessionClosed: Story = {
  render: () => (
    <GalleryRow
      label="Notice / SessionClosed"
      chat={<ChatNoticeBlock block={noticeBlocks.sessionClosed} />}
      dev={<DevNoticeBlock block={noticeBlocks.sessionClosed} />}
      devJson={<DevNoticeBlock block={noticeBlocks.sessionClosed} />}
    />
  ),
}

export const NoticeAssistantError: Story = {
  render: () => (
    <GalleryRow
      label="Notice / AssistantError (billing)"
      chat={<ChatNoticeBlock block={noticeBlocks.billingError} />}
      dev={<DevNoticeBlock block={noticeBlocks.billingError} />}
      devJson={<DevNoticeBlock block={noticeBlocks.billingError} />}
    />
  ),
}

// ── Interaction Blocks ─────────────────────────────────────────────────────

export const InteractionPermission: Story = {
  render: () => (
    <GalleryRow
      label="Interaction / Permission (pending)"
      chat={<ChatInteractionBlock block={interactionBlocks.permissionPending} />}
      dev={<DevInteractionBlock block={interactionBlocks.permissionPending} />}
      devJson={<DevInteractionBlock block={interactionBlocks.permissionPending} />}
    />
  ),
}

export const InteractionElicitation: Story = {
  render: () => (
    <GalleryRow
      label="Interaction / Elicitation"
      chat={<ChatInteractionBlock block={interactionBlocks.elicitationPending} />}
      dev={<DevInteractionBlock block={interactionBlocks.elicitationPending} />}
      devJson={<DevInteractionBlock block={interactionBlocks.elicitationPending} />}
    />
  ),
}

export const InteractionQuestion: Story = {
  render: () => (
    <GalleryRow
      label="Interaction / Question"
      chat={<ChatInteractionBlock block={interactionBlocks.questionPending} />}
      dev={<DevInteractionBlock block={interactionBlocks.questionPending} />}
      devJson={<DevInteractionBlock block={interactionBlocks.questionPending} />}
    />
  ),
}

export const InteractionPlan: Story = {
  render: () => (
    <GalleryRow
      label="Interaction / Plan"
      chat={<ChatInteractionBlock block={interactionBlocks.planPending} />}
      dev={<DevInteractionBlock block={interactionBlocks.planPending} />}
      devJson={<DevInteractionBlock block={interactionBlocks.planPending} />}
    />
  ),
}

// ── Progress Blocks ────────────────────────────────────────────────────────

export const ProgressBash: Story = {
  render: () => (
    <GalleryRow
      label="Progress / Bash"
      chat={<ChatProgressBlock block={progressBlocks.bash} />}
      dev={<DevProgressBlock block={progressBlocks.bash} />}
      devJson={<DevProgressBlock block={progressBlocks.bash} />}
    />
  ),
}

export const ProgressHook: Story = {
  render: () => (
    <GalleryRow
      label="Progress / Hook"
      chat={<ChatProgressBlock block={progressBlocks.hook} />}
      dev={<DevProgressBlock block={progressBlocks.hook} />}
      devJson={<DevProgressBlock block={progressBlocks.hook} />}
    />
  ),
}

export const ProgressAgent: Story = {
  render: () => (
    <GalleryRow
      label="Progress / Agent"
      chat={<ChatProgressBlock block={progressBlocks.agent} />}
      dev={<DevProgressBlock block={progressBlocks.agent} />}
      devJson={<DevProgressBlock block={progressBlocks.agent} />}
    />
  ),
}

export const ProgressMcp: Story = {
  render: () => (
    <GalleryRow
      label="Progress / MCP"
      chat={<ChatProgressBlock block={progressBlocks.mcp} />}
      dev={<DevProgressBlock block={progressBlocks.mcp} />}
      devJson={<DevProgressBlock block={progressBlocks.mcp} />}
    />
  ),
}
