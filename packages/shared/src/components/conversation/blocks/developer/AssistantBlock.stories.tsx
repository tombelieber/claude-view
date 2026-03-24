import type { Meta, StoryObj } from '@storybook/react-vite'
import { assistantBlocks } from '../../../../stories/fixtures'
import { devAssistantBlocks } from '../../../../stories/fixtures-developer'
import { DevAssistantBlock } from './AssistantBlock'

const meta = {
  title: 'Developer/Blocks/AssistantBlock',
  component: DevAssistantBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevAssistantBlock>

export default meta
type Story = StoryObj<typeof meta>

export const TextOnly: Story = {
  args: { block: assistantBlocks.textOnly },
}

export const Streaming: Story = {
  args: { block: assistantBlocks.streaming },
}

export const WithThinking: Story = {
  args: { block: assistantBlocks.withThinking },
}

export const WithTools: Story = {
  args: { block: assistantBlocks.withTools },
}

export const WithRawJson: Story = {
  args: { block: devAssistantBlocks.withRawJson },
}

export const WithPermissionMode: Story = {
  args: { block: devAssistantBlocks.withPermissionMode },
}
