import type { Meta, StoryObj } from '@storybook/react-vite'
import type { ProgressBlock } from '../../../../types/blocks'
import { agentGroupBlocks } from '../../../../stories/fixtures'
import { DevAgentGroupRow } from './AgentGroupRow'

const meta = {
  title: 'Chat/Blocks/Developer/AgentGroupRow',
  component: DevAgentGroupRow,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof DevAgentGroupRow>

export default meta
type Story = StoryObj<typeof meta>

/** Full 10-message agent group — Glob, Grep, Read ×3, Bash. */
export const Default: Story = {
  args: { blocks: agentGroupBlocks as ProgressBlock[] },
}

/** Small group (3 messages) — minimum threshold for collapsing. */
export const SmallGroup: Story = {
  args: { blocks: agentGroupBlocks.slice(0, 3) as ProgressBlock[] },
}
