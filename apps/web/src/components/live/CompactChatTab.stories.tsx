import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../stories/decorators'
import { errorConversation, fullConversation } from '../../stories/fixtures'
import { CompactChatTab } from './CompactChatTab'

const meta = {
  title: 'Live/CompactChatTab',
  component: CompactChatTab,
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof CompactChatTab>

export default meta
type Story = StoryObj<typeof meta>

export const FullConversation: Story = { args: { blocks: fullConversation } }
export const ErrorConversation: Story = { args: { blocks: errorConversation } }
export const Empty: Story = { args: { blocks: [] } }
