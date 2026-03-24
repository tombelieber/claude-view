import type { Meta, StoryObj } from '@storybook/react-vite'
import { MessageTimestamp } from './MessageTimestamp'

const meta = {
  title: 'Chat/Shared/MessageTimestamp',
  component: MessageTimestamp,
  parameters: { layout: 'centered' },
} satisfies Meta<typeof MessageTimestamp>

export default meta
type Story = StoryObj<typeof meta>

const NOW = Math.floor(Date.now() / 1000)

export const Recent: Story = {
  args: { timestamp: NOW - 60 },
}

export const HoursAgo: Story = {
  args: { timestamp: NOW - 3600 * 3 },
}

export const AlignRight: Story = {
  args: { timestamp: NOW - 300, align: 'right' },
}

export const NoTimestamp: Story = {
  args: { timestamp: undefined },
}

export const Zero: Story = {
  args: { timestamp: 0 },
}
