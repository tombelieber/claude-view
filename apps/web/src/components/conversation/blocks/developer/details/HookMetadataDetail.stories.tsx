import type { Meta, StoryObj } from '@storybook/react-vite'
import { rawJsonFixtures } from '../../../../../stories/fixtures-developer'
import { HookMetadataDetail } from './HookMetadataDetail'

const meta = {
  title: 'Developer/Details/HookMetadataDetail',
  component: HookMetadataDetail,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof HookMetadataDetail>

export default meta
type Story = StoryObj<typeof meta>

export const WithHooks: Story = {
  args: { rawJson: rawJsonFixtures.withHooks },
}

export const WithErrors: Story = {
  args: { rawJson: rawJsonFixtures.withHookErrors },
}

export const NoData: Story = {
  args: { rawJson: null },
}
