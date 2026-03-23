import type { Meta, StoryObj } from '@storybook/react-vite'
import { toolExecutions } from '../../../../stories/fixtures'
import { ToolChip } from './ToolChip'

const meta = {
  title: 'Chat/Shared/ToolChip',
  component: ToolChip,
  parameters: { layout: 'centered' },
} satisfies Meta<typeof ToolChip>

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

export const GrepRunning: Story = {
  args: { execution: toolExecutions.grepRunning },
}
