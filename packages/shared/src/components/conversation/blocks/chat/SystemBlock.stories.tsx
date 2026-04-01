import type { Meta, StoryObj } from '@storybook/react-vite'
import { systemBlocks } from '../../../../stories/fixtures'
import { ChatSystemBlock } from './SystemBlock'

const meta = {
  title: 'Chat/Blocks/SystemBlock',
  component: ChatSystemBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatSystemBlock>

export default meta
type Story = StoryObj<typeof meta>

export const TaskStarted: Story = {
  args: { block: systemBlocks.taskStarted },
}

export const TaskProgress: Story = {
  args: { block: systemBlocks.taskProgress },
}

export const TaskCompleted: Story = {
  args: { block: systemBlocks.taskCompleted },
}

export const TaskFailed: Story = {
  args: { block: systemBlocks.taskFailed },
}

export const QueueOperation: Story = {
  args: { block: systemBlocks.queueOperation },
}

export const PrLink: Story = {
  args: { block: systemBlocks.prLink },
}

export const CustomTitle: Story = {
  args: { block: systemBlocks.customTitle },
}

export const PlanContent: Story = {
  args: { block: systemBlocks.planContent },
}

export const AgentName: Story = {
  args: { block: systemBlocks.agentName },
}
