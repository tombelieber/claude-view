import type { Meta, StoryObj } from '@storybook/react-vite'
import { withChatContext } from '../../../../stories/decorators'
import { interactionBlocks } from '../../../../stories/fixtures'
import { ChatInteractionBlock } from './InteractionBlock'

const meta = {
  title: 'Chat/Blocks/Chat/InteractionBlock',
  component: ChatInteractionBlock,
  decorators: [withChatContext],
  parameters: { layout: 'padded' },
} satisfies Meta<typeof ChatInteractionBlock>

export default meta
type Story = StoryObj<typeof meta>

export const PermissionPending: Story = { args: { block: interactionBlocks.permissionPending } }
export const PermissionResolved: Story = { args: { block: interactionBlocks.permissionResolved } }
export const QuestionPending: Story = { args: { block: interactionBlocks.questionPending } }
export const PlanPending: Story = { args: { block: interactionBlocks.planPending } }
export const ElicitationPending: Story = { args: { block: interactionBlocks.elicitationPending } }
