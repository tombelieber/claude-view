import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { ThinkingMetadataDetail } from './ThinkingMetadataDetail'

const meta = {
  title: 'Developer/Details/ThinkingMetadataDetail',
  component: ThinkingMetadataDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ThinkingMetadataDetail>

export default meta
type Story = StoryObj<typeof meta>

export const WithMetadata: Story = {
  args: { rawJson: rawJsonFixtures.withThinkingMetadata },
}

export const NoData: Story = {
  args: { rawJson: null },
}
