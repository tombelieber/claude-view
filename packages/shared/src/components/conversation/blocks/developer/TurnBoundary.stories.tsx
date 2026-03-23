import type { Meta, StoryObj } from '@storybook/react-vite'
import { turnBoundaryBlocks } from '../../../../stories/fixtures'
import { devTurnBoundaryBlocks } from '../../../../stories/fixtures-developer'
import { DevTurnBoundary } from './TurnBoundary'

const meta = {
  title: 'Developer/Blocks/TurnBoundary',
  component: DevTurnBoundary,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevTurnBoundary>

export default meta
type Story = StoryObj<typeof meta>

export const Success: Story = {
  args: { block: turnBoundaryBlocks.success },
}

export const ErrorTurn: Story = {
  args: { block: turnBoundaryBlocks.error },
}

export const MaxTurns: Story = {
  args: { block: turnBoundaryBlocks.maxTurns },
}

export const WithPermissionDenials: Story = {
  args: { block: devTurnBoundaryBlocks.withPermissionDenials },
}
