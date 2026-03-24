import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { ApiErrorDetail } from './ApiErrorDetail'

const meta = {
  title: 'Developer/Details/ApiErrorDetail',
  component: ApiErrorDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ApiErrorDetail>

export default meta
type Story = StoryObj<typeof meta>

export const WithError: Story = {
  args: { rawJson: rawJsonFixtures.withApiError },
}

export const NoError: Story = {
  args: { rawJson: null },
}

export const EmptyObject: Story = {
  args: { rawJson: rawJsonFixtures.empty },
}
