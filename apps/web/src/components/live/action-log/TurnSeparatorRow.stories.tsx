import type { Meta, StoryObj } from '@storybook/react-vite'
import { TurnSeparatorRow } from './TurnSeparatorRow'

const meta = {
  title: 'Live/ActionLog/TurnSeparatorRow',
  component: TurnSeparatorRow,
  parameters: { layout: 'padded', backgrounds: { default: 'dark' } },
} satisfies Meta<typeof TurnSeparatorRow>

export default meta
type Story = StoryObj<typeof meta>

export const UserTurn: Story = {
  args: { role: 'user', content: 'Can you help me refactor the auth middleware?' },
}

export const AssistantTurn: Story = {
  args: { role: 'assistant', content: "I'll start by reading the current implementation..." },
}

export const LongContent: Story = {
  args: {
    role: 'user',
    content:
      'I need you to do several things: first read the WebSocket handler, then refactor it to use event-driven patterns, make sure tests pass, and update documentation.',
  },
}
