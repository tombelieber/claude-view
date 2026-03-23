import type { Meta, StoryObj } from '@storybook/react-vite'
import { withConversationActions } from '../../../../stories/decorators'
import { interactionBlocks } from '../../../../stories/fixtures'
import { DevInteractionBlock } from './InteractionBlock'

const meta = {
  title: 'Developer/Blocks/InteractionBlock',
  component: DevInteractionBlock,
  parameters: { layout: 'padded' },
  decorators: [
    withConversationActions,
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevInteractionBlock>

export default meta
type Story = StoryObj<typeof meta>

export const PermissionPending: Story = {
  args: { block: interactionBlocks.permissionPending },
}

export const PermissionResolved: Story = {
  args: { block: interactionBlocks.permissionResolved },
}

export const Question: Story = {
  args: { block: interactionBlocks.questionPending },
}

export const Plan: Story = {
  args: { block: interactionBlocks.planPending },
}

export const Elicitation: Story = {
  args: { block: interactionBlocks.elicitationPending },
}
