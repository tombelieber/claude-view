import type { Meta, StoryObj } from '@storybook/react-vite'
import { DayDivider } from './DayDivider'

const meta = {
  title: 'Chat/Shared/DayDivider',
  component: DayDivider,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DayDivider>

export default meta
type Story = StoryObj<typeof meta>

export const Today: Story = {
  args: { label: 'Today' },
}

export const Yesterday: Story = {
  args: { label: 'Yesterday' },
}

export const Weekday: Story = {
  args: { label: 'Monday' },
}

export const OlderDate: Story = {
  args: { label: 'Sat, Mar 15' },
}
