import type { Meta, StoryObj } from '@storybook/react-vite'
import { noticeBlocks } from '../../../../stories/fixtures'
import { ChatNoticeBlock } from './NoticeBlock'

const meta = {
  title: 'Chat/Blocks/NoticeBlock',
  component: ChatNoticeBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatNoticeBlock>

export default meta
type Story = StoryObj<typeof meta>

export const RateLimitError: Story = {
  args: { block: noticeBlocks.assistantError },
}

export const BillingError: Story = {
  args: { block: noticeBlocks.billingError },
}

export const AuthenticationFailed: Story = {
  args: { block: noticeBlocks.authFailed },
}

export const ServerError: Story = {
  args: { block: noticeBlocks.serverError },
}

export const RateLimitWarning: Story = {
  args: { block: noticeBlocks.rateLimitWarning },
}

export const RateLimitRejected: Story = {
  args: { block: noticeBlocks.rateLimitRejected },
}

export const ContextCompactedAuto: Story = {
  args: { block: noticeBlocks.contextCompacted },
}

export const ContextCompactedManual: Story = {
  args: { block: noticeBlocks.contextCompactedManual },
}

export const Authenticating: Story = {
  args: { block: noticeBlocks.authenticating },
}

export const AuthError: Story = {
  args: { block: noticeBlocks.authError },
}

export const SessionClosed: Story = {
  args: { block: noticeBlocks.sessionClosed },
}

export const NonFatalError: Story = {
  args: { block: noticeBlocks.error },
}

export const FatalError: Story = {
  args: { block: noticeBlocks.fatalError },
}

export const PromptSuggestion: Story = {
  args: { block: noticeBlocks.promptSuggestion },
}

export const SessionResumed: Story = {
  args: { block: noticeBlocks.sessionResumed },
}

export const RateLimitWithRetry: Story = {
  args: { block: noticeBlocks.rateLimitWithRetry },
}
