import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../stories/decorators'
import { errorConversation, fullConversation } from '../../stories/fixtures'
import { ConversationThread } from './ConversationThread'
import { ChatAssistantBlock } from './blocks/chat/AssistantBlock'
import { ChatInteractionBlock } from './blocks/chat/InteractionBlock'
import { ChatNoticeBlock } from './blocks/chat/NoticeBlock'
import { ChatSystemBlock } from './blocks/chat/SystemBlock'
import { ChatTurnBoundary } from './blocks/chat/TurnBoundary'
import { ChatUserBlock } from './blocks/chat/UserBlock'
import type { BlockRenderers } from './types'

const chatRenderers: BlockRenderers = {
  user: ChatUserBlock as BlockRenderers['user'],
  assistant: ChatAssistantBlock as BlockRenderers['assistant'],
  notice: ChatNoticeBlock as BlockRenderers['notice'],
  turn_boundary: ChatTurnBoundary as BlockRenderers['turn_boundary'],
  system: ChatSystemBlock as BlockRenderers['system'],
  interaction: ChatInteractionBlock as BlockRenderers['interaction'],
}

const meta = {
  title: 'Chat/ConversationThread',
  component: ConversationThread,
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof ConversationThread>

export default meta
type Story = StoryObj<typeof meta>

export const FullConversation: Story = {
  args: {
    blocks: fullConversation,
    renderers: chatRenderers,
  },
}

export const ErrorConversation: Story = {
  args: {
    blocks: errorConversation,
    renderers: chatRenderers,
  },
}

export const Compact: Story = {
  args: {
    blocks: fullConversation,
    renderers: chatRenderers,
    compact: true,
  },
}

export const Empty: Story = {
  args: {
    blocks: [],
    renderers: chatRenderers,
  },
}
