import type { Meta, StoryObj } from '@storybook/react-vite'
import { noticeBlocks } from '../../../../stories/fixtures'
import { DevNoticeBlock } from './NoticeBlock'

const meta = {
  title: 'Developer/Blocks/NoticeBlock',
  component: DevNoticeBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevNoticeBlock>

export default meta
type Story = StoryObj<typeof meta>

export const AssistantError: Story = {
  args: { block: noticeBlocks.assistantError },
}

export const BillingError: Story = {
  args: { block: noticeBlocks.billingError },
}

export const AuthFailed: Story = {
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

export const ContextCompacted: Story = {
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

export const Error: Story = {
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
