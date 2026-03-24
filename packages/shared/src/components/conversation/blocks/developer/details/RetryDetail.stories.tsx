import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { RetryDetail } from './RetryDetail'

const meta = {
  title: 'Developer/Details/RetryDetail',
  component: RetryDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof RetryDetail>

export default meta
type Story = StoryObj<typeof meta>

export const Retrying: Story = {
  args: { rawJson: rawJsonFixtures.withRetry },
}

export const NoRetry: Story = {
  args: { rawJson: null },
}
