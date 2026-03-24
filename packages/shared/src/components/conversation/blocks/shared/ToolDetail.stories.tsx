import type { Meta, StoryObj } from '@storybook/react-vite'
import { toolExecutions } from '../../../../stories/fixtures'
import { ToolDetail } from './ToolDetail'

const meta = {
  title: 'Chat/Shared/ToolDetail',
  component: ToolDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ToolDetail>

export default meta
type Story = StoryObj<typeof meta>

export const BashRunning: Story = {
  args: { execution: toolExecutions.bashRunning },
}

export const BashComplete: Story = {
  args: { execution: toolExecutions.bashComplete },
}

export const BashError: Story = {
  args: { execution: toolExecutions.bashError },
}

export const ReadComplete: Story = {
  args: { execution: toolExecutions.readComplete },
}

export const EditComplete: Story = {
  args: { execution: toolExecutions.editComplete },
}
