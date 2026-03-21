import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../stories/decorators'
import { errorConversation, fullConversation } from '../../stories/fixtures'
import { ConversationThread } from './ConversationThread'
import { chatRegistry } from './blocks/chat/registry'
import { developerRegistry } from './blocks/developer/registry'

const meta = {
  title: 'Chat/ConversationThread',
  component: ConversationThread,
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof ConversationThread>

export default meta
type Story = StoryObj<typeof meta>

/** Chat mode — all 7 block types rendered with chat registry. */
export const ChatMode: Story = {
  args: {
    blocks: fullConversation,
    renderers: chatRegistry,
  },
}

/** Developer mode — all 7 block types rendered with developer registry. */
export const DeveloperMode: Story = {
  args: {
    blocks: fullConversation,
    renderers: developerRegistry,
  },
}

/** Error conversation — notices and failed turns. */
export const ErrorConversation: Story = {
  args: {
    blocks: errorConversation,
    renderers: chatRegistry,
  },
}

/** Compact layout — used in Live Monitor side panel. */
export const Compact: Story = {
  args: {
    blocks: fullConversation,
    renderers: chatRegistry,
    compact: true,
  },
}

/** Empty state. */
export const Empty: Story = {
  args: {
    blocks: [],
    renderers: chatRegistry,
  },
}
