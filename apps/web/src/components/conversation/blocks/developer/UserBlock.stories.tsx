import type { Meta, StoryObj } from '@storybook/react-vite'
import { userBlocks } from '../../../../stories/fixtures'
import { devUserBlocks } from '../../../../stories/fixtures-developer'
import { DevUserBlock } from './UserBlock'

const meta = {
  title: 'Developer/Blocks/UserBlock',
  component: DevUserBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevUserBlock>

export default meta
type Story = StoryObj<typeof meta>

export const Sent: Story = {
  args: { block: userBlocks.sent },
}

export const Failed: Story = {
  args: { block: userBlocks.failed },
}

export const WithRawJson: Story = {
  args: { block: devUserBlocks.withRawJson },
}

export const WithImagePastes: Story = {
  args: { block: devUserBlocks.withImagePastes },
}
