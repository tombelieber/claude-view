import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../../../stories/decorators'
import { userBlocks } from '../../../../stories/fixtures'
import { ChatUserBlock } from './UserBlock'

const meta = {
  title: 'Chat/Blocks/UserBlock',
  component: ChatUserBlock,
  decorators: [withChatContext],
} satisfies Meta<typeof ChatUserBlock>

export default meta
type Story = StoryObj<typeof meta>

export const Sent: Story = {
  args: { block: userBlocks.sent },
}

export const Normal: Story = {
  args: { block: userBlocks.normal },
}

export const Optimistic: Story = {
  args: { block: userBlocks.optimistic },
}

export const Sending: Story = {
  args: { block: userBlocks.sending },
}

export const Failed: Story = {
  args: { block: userBlocks.failed },
}

export const LongMessage: Story = {
  args: { block: userBlocks.long },
}
