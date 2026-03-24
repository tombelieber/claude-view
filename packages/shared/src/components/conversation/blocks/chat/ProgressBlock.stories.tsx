import type { Meta, StoryObj } from '@storybook/react-vite'
import { progressBlocks } from '../../../../stories/fixtures'
import { ChatProgressBlock } from './ProgressBlock'

const meta = {
  title: 'Chat/Blocks/Chat/ProgressBlock',
  component: ChatProgressBlock,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof ChatProgressBlock>

export default meta
type Story = StoryObj<typeof meta>

export const Bash: Story = { args: { block: progressBlocks.bash } }
export const Agent: Story = { args: { block: progressBlocks.agent } }
export const Hook: Story = { args: { block: progressBlocks.hook } }
export const Mcp: Story = { args: { block: progressBlocks.mcp } }
export const TaskQueue: Story = { args: { block: progressBlocks.taskQueue } }
export const Search: Story = { args: { block: progressBlocks.search } }
export const Query: Story = { args: { block: progressBlocks.query } }
