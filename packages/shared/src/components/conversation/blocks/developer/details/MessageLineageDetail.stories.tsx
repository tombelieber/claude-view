import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { MessageLineageDetail } from './MessageLineageDetail'

const meta = {
  title: 'Developer/Details/MessageLineageDetail',
  component: MessageLineageDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof MessageLineageDetail>

export default meta
type Story = StoryObj<typeof meta>

export const WithLineage: Story = {
  args: { rawJson: rawJsonFixtures.withLineage },
}

export const NoData: Story = {
  args: { rawJson: null },
}
