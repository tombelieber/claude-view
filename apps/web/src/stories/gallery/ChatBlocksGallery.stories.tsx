import type { Meta, StoryObj } from '@storybook/react-vite'
import { ChatAssistantBlock } from '../../components/conversation/blocks/chat/AssistantBlock'
import { ChatNoticeBlock } from '../../components/conversation/blocks/chat/NoticeBlock'
import { ChatSystemBlock } from '../../components/conversation/blocks/chat/SystemBlock'
import { ChatTurnBoundary } from '../../components/conversation/blocks/chat/TurnBoundary'
import { ChatUserBlock } from '../../components/conversation/blocks/chat/UserBlock'
import { withChatContext } from '../decorators'
import {
  assistantBlocks,
  noticeBlocks,
  systemBlocks,
  turnBoundaryBlocks,
  userBlocks,
} from '../fixtures'

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
      <Section title="UserBlock">
        <Label>sent</Label>
        <ChatUserBlock block={userBlocks.sent} />
        <Label>optimistic</Label>
        <ChatUserBlock block={userBlocks.optimistic} />
        <Label>sending</Label>
        <ChatUserBlock block={userBlocks.sending} />
        <Label>failed</Label>
        <ChatUserBlock block={userBlocks.failed} />
        <Label>long message</Label>
        <ChatUserBlock block={userBlocks.long} />
      </Section>

      <Section title="AssistantBlock">
        <Label>text only</Label>
        <ChatAssistantBlock block={assistantBlocks.textOnly} />
        <Label>streaming</Label>
        <ChatAssistantBlock block={assistantBlocks.streaming} />
        <Label>with thinking</Label>
        <ChatAssistantBlock block={assistantBlocks.withThinking} />
        <Label>with tools</Label>
        <ChatAssistantBlock block={assistantBlocks.withTools} />
        <Label>with running tool</Label>
        <ChatAssistantBlock block={assistantBlocks.withRunningTool} />
        <Label>with tool error</Label>
        <ChatAssistantBlock block={assistantBlocks.withToolError} />
        <Label>rich markdown</Label>
        <ChatAssistantBlock block={assistantBlocks.markdown} />
      </Section>

      <Section title="NoticeBlock">
        <div className="grid grid-cols-1 gap-2">
          <Label>assistant errors</Label>
          <ChatNoticeBlock block={noticeBlocks.assistantError} />
          <ChatNoticeBlock block={noticeBlocks.billingError} />
          <ChatNoticeBlock block={noticeBlocks.authFailed} />
          <ChatNoticeBlock block={noticeBlocks.serverError} />
          <Label>rate limits</Label>
          <ChatNoticeBlock block={noticeBlocks.rateLimitWarning} />
          <ChatNoticeBlock block={noticeBlocks.rateLimitRejected} />
          <Label>system notices</Label>
          <ChatNoticeBlock block={noticeBlocks.contextCompacted} />
          <ChatNoticeBlock block={noticeBlocks.authenticating} />
          <ChatNoticeBlock block={noticeBlocks.authError} />
          <ChatNoticeBlock block={noticeBlocks.sessionClosed} />
          <ChatNoticeBlock block={noticeBlocks.error} />
          <ChatNoticeBlock block={noticeBlocks.fatalError} />
          <ChatNoticeBlock block={noticeBlocks.promptSuggestion} />
          <ChatNoticeBlock block={noticeBlocks.sessionResumed} />
        </div>
      </Section>

      <Section title="TurnBoundary">
        <div className="space-y-2">
          <Label>success ($0.034)</Label>
          <ChatTurnBoundary block={turnBoundaryBlocks.success} />
          <Label>cheap ($0.0008)</Label>
          <ChatTurnBoundary block={turnBoundaryBlocks.cheap} />
          <Label>free</Label>
          <ChatTurnBoundary block={turnBoundaryBlocks.free} />
          <Label>error</Label>
          <ChatTurnBoundary block={turnBoundaryBlocks.error} />
          <Label>max turns</Label>
          <ChatTurnBoundary block={turnBoundaryBlocks.maxTurns} />
        </div>
      </Section>

      <Section title="SystemBlock">
        <ChatSystemBlock block={systemBlocks.taskStarted} />
        <ChatSystemBlock block={systemBlocks.taskProgress} />
        <ChatSystemBlock block={systemBlocks.taskCompleted} />
        <ChatSystemBlock block={systemBlocks.taskFailed} />
      </Section>
    </div>
  )
}

const meta = {
  title: 'Gallery/Chat Blocks',
  component: Gallery,
  tags: [],
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof Gallery>

export default meta
type Story = StoryObj<typeof meta>

export const AllVariants: Story = {}
