import type { Meta, StoryObj } from '@storybook/react-vite'
import { turnBoundaryBlocks } from '../../../../stories/fixtures'
import { ChatTurnBoundary } from './TurnBoundary'

const meta = {
  title: 'Chat/Blocks/TurnBoundary',
  component: ChatTurnBoundary,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatTurnBoundary>

export default meta
type Story = StoryObj<typeof meta>

export const Success: Story = {
  args: { block: turnBoundaryBlocks.success },
}

export const Cheap: Story = {
  args: { block: turnBoundaryBlocks.cheap },
}

export const Free: Story = {
  args: { block: turnBoundaryBlocks.free },
}

export const ErrorTurn: Story = {
  args: { block: turnBoundaryBlocks.error },
}

export const MaxTurns: Story = {
  args: { block: turnBoundaryBlocks.maxTurns },
}

export const WithHookErrors: Story = {
  args: { block: turnBoundaryBlocks.withHookErrors },
}

export const PreventedContinuation: Story = {
  args: { block: turnBoundaryBlocks.preventedContinuation },
}
