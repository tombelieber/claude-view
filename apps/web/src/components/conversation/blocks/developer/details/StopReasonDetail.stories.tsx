import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { StopReasonDetail } from './StopReasonDetail'

const meta = {
  title: 'Developer/Details/StopReasonDetail',
  component: StopReasonDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof StopReasonDetail>

export default meta
type Story = StoryObj<typeof meta>

export const EndTurn: Story = {
  args: { rawJson: rawJsonFixtures.withStopReason },
}

export const MaxTokensPrevented: Story = {
  args: { rawJson: rawJsonFixtures.withStopReasonPrevented },
}

export const NoData: Story = {
  args: { rawJson: null },
}
