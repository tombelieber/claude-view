import type { Meta, StoryObj } from '@storybook/react-vite'
import { assistantBlocks } from '../../../../stories/fixtures'
import { ChatAssistantBlock } from './AssistantBlock'

const meta = {
  title: 'Chat/Blocks/AssistantBlock',
  component: ChatAssistantBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatAssistantBlock>

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

export const WithRunningTool: Story = {
  args: { block: assistantBlocks.withRunningTool },
}

export const WithToolError: Story = {
  args: { block: assistantBlocks.withToolError },
}

export const RichMarkdown: Story = {
  args: { block: assistantBlocks.markdown },
}

export const SidechainReply: Story = {
  args: { block: assistantBlocks.sidechainReply },
}

export const FromAgent: Story = {
  args: { block: assistantBlocks.fromAgent },
}
