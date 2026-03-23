import type { Meta, StoryObj } from '@storybook/react-vite'
import { withConversationActions } from '../../../../stories/decorators'
import { systemBlocks } from '../../../../stories/fixtures'
import { devSystemBlocks, devSystemBlocksWithRawJson } from '../../../../stories/fixtures-developer'
import { DevSystemBlock } from './SystemBlock'

const meta = {
  title: 'Developer/Blocks/SystemBlock',
  component: DevSystemBlock,
  parameters: { layout: 'padded' },
  decorators: [
    withConversationActions,
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof DevSystemBlock>

export default meta
type Story = StoryObj<typeof meta>

export const SessionInit: Story = {
  args: { block: devSystemBlocks.sessionInit },
}

export const SessionStatus: Story = {
  args: { block: devSystemBlocks.sessionStatus },
}

export const SessionStatusIdle: Story = {
  args: { block: devSystemBlocks.sessionStatusIdle },
}

export const ElicitationComplete: Story = {
  args: { block: devSystemBlocks.elicitationComplete },
}

export const HookEvent: Story = {
  args: { block: devSystemBlocks.hookEvent },
}

export const HookEventError: Story = {
  args: { block: devSystemBlocks.hookEventError },
}

export const TaskStarted: Story = {
  args: { block: systemBlocks.taskStarted },
}

export const TaskProgress: Story = {
  args: { block: systemBlocks.taskProgress },
}

export const TaskCompleted: Story = {
  args: { block: systemBlocks.taskCompleted },
}

export const TaskFailed: Story = {
  args: { block: systemBlocks.taskFailed },
}

export const FilesSaved: Story = {
  args: { block: devSystemBlocks.filesSaved },
}

export const FilesSavedWithFailures: Story = {
  args: { block: devSystemBlocks.filesSavedWithFailures },
}

export const CommandOutput: Story = {
  args: { block: devSystemBlocks.commandOutput },
}

export const StreamDeltaEvent: Story = {
  args: { block: devSystemBlocks.streamDelta },
}

export const Unknown: Story = {
  args: { block: devSystemBlocks.unknown },
}

export const LocalCommand: Story = {
  args: { block: devSystemBlocks.localCommand },
}

export const QueueOperation: Story = {
  args: { block: devSystemBlocks.queueOperation },
}

export const FileHistorySnapshot: Story = {
  args: { block: devSystemBlocks.fileHistorySnapshot },
}

export const AiTitle: Story = {
  args: { block: devSystemBlocks.aiTitle },
}

export const LastPrompt: Story = {
  args: { block: devSystemBlocks.lastPrompt },
}

export const Informational: Story = {
  args: { block: devSystemBlocks.informational },
}

export const WithRetryRawJson: Story = {
  args: { block: devSystemBlocksWithRawJson.withRetry },
}

export const WithApiErrorRawJson: Story = {
  args: { block: devSystemBlocksWithRawJson.withApiError },
}

export const WithHooksRawJson: Story = {
  args: { block: devSystemBlocksWithRawJson.withHooks },
}

export const WithHookErrorsRawJson: Story = {
  args: { block: devSystemBlocksWithRawJson.withHookErrors },
}

export const WithAllRawJson: Story = {
  args: { block: devSystemBlocksWithRawJson.withAll },
}
