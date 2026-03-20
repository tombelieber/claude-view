import type { Meta, StoryObj } from '@storybook/react-vite'
import { DevAssistantBlock } from '../../components/conversation/blocks/developer/AssistantBlock'
import { DevNoticeBlock } from '../../components/conversation/blocks/developer/NoticeBlock'
import { DevSystemBlock } from '../../components/conversation/blocks/developer/SystemBlock'
import { DevTurnBoundary } from '../../components/conversation/blocks/developer/TurnBoundary'
import { DevUserBlock } from '../../components/conversation/blocks/developer/UserBlock'
import { withChatContext } from '../decorators'
import {
  assistantBlocks,
  noticeBlocks,
  systemBlocks,
  turnBoundaryBlocks,
  userBlocks,
} from '../fixtures'
import {
  devAssistantBlocks,
  devSystemBlocks,
  devTurnBoundaryBlocks,
  devUserBlocks,
} from '../fixtures-developer'

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="space-y-2">
      <h3 className="text-[11px] font-mono font-semibold uppercase tracking-widest text-gray-400 dark:text-gray-500 border-b border-gray-200 dark:border-gray-700 pb-1">
        {title}
      </h3>
      <div className="space-y-3">{children}</div>
    </div>
  )
}

function Label({ children }: { children: React.ReactNode }) {
  return (
    <span className="text-[9px] font-mono text-gray-400 dark:text-gray-500 uppercase tracking-wide">
      {children}
    </span>
  )
}

function Gallery() {
  return (
    <div className="w-[680px] max-w-full space-y-8">
      <Section title="DevUserBlock">
        <Label>sent</Label>
        <DevUserBlock block={userBlocks.sent} />
        <Label>failed</Label>
        <DevUserBlock block={userBlocks.failed} />
        <Label>with rawJson lineage</Label>
        <DevUserBlock block={devUserBlocks.withRawJson} />
      </Section>

      <Section title="DevAssistantBlock">
        <Label>text only</Label>
        <DevAssistantBlock block={assistantBlocks.textOnly} />
        <Label>streaming</Label>
        <DevAssistantBlock block={assistantBlocks.streaming} />
        <Label>with thinking</Label>
        <DevAssistantBlock block={assistantBlocks.withThinking} />
        <Label>with tools</Label>
        <DevAssistantBlock block={assistantBlocks.withTools} />
        <Label>with rawJson metadata</Label>
        <DevAssistantBlock block={devAssistantBlocks.withRawJson} />
      </Section>

      <Section title="DevNoticeBlock">
        <div className="space-y-2">
          <Label>errors</Label>
          <DevNoticeBlock block={noticeBlocks.assistantError} />
          <DevNoticeBlock block={noticeBlocks.billingError} />
          <Label>rate limits</Label>
          <DevNoticeBlock block={noticeBlocks.rateLimitWarning} />
          <DevNoticeBlock block={noticeBlocks.rateLimitRejected} />
          <Label>system</Label>
          <DevNoticeBlock block={noticeBlocks.contextCompacted} />
          <DevNoticeBlock block={noticeBlocks.authenticating} />
          <DevNoticeBlock block={noticeBlocks.authError} />
          <DevNoticeBlock block={noticeBlocks.sessionClosed} />
          <DevNoticeBlock block={noticeBlocks.error} />
          <DevNoticeBlock block={noticeBlocks.fatalError} />
          <DevNoticeBlock block={noticeBlocks.sessionResumed} />
        </div>
      </Section>

      <Section title="DevTurnBoundary">
        <Label>success</Label>
        <DevTurnBoundary block={turnBoundaryBlocks.success} />
        <Label>error</Label>
        <DevTurnBoundary block={turnBoundaryBlocks.error} />
        <Label>max turns</Label>
        <DevTurnBoundary block={turnBoundaryBlocks.maxTurns} />
        <Label>with permission denials</Label>
        <DevTurnBoundary block={devTurnBoundaryBlocks.withPermissionDenials} />
      </Section>

      <Section title="DevSystemBlock">
        <Label>session init</Label>
        <DevSystemBlock block={devSystemBlocks.sessionInit} />
        <Label>session status</Label>
        <DevSystemBlock block={devSystemBlocks.sessionStatus} />
        <Label>hook event</Label>
        <DevSystemBlock block={devSystemBlocks.hookEvent} />
        <Label>task started</Label>
        <DevSystemBlock block={systemBlocks.taskStarted} />
        <Label>task progress</Label>
        <DevSystemBlock block={systemBlocks.taskProgress} />
        <Label>task completed</Label>
        <DevSystemBlock block={systemBlocks.taskCompleted} />
        <Label>files saved</Label>
        <DevSystemBlock block={devSystemBlocks.filesSaved} />
        <Label>command output</Label>
        <DevSystemBlock block={devSystemBlocks.commandOutput} />
      </Section>
    </div>
  )
}

const meta = {
  title: 'Gallery/Developer Blocks',
  component: Gallery,
  tags: [],
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof Gallery>

export default meta
type Story = StoryObj<typeof meta>

export const AllVariants: Story = {}
