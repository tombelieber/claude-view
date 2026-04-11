import type { Meta, StoryObj } from '@storybook/react-vite'
import { systemBlocks } from '../../../../stories/fixtures'
import { ChatSystemBlock } from './SystemBlock'

const meta = {
  title: 'Chat/Blocks/SystemBlock',
  component: ChatSystemBlock,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[640px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatSystemBlock>

export default meta
type Story = StoryObj<typeof meta>

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

export const QueueOperation: Story = {
  args: { block: systemBlocks.queueOperation },
}

export const PrLink: Story = {
  args: { block: systemBlocks.prLink },
}

export const CustomTitle: Story = {
  args: { block: systemBlocks.customTitle },
}

export const PlanContent: Story = {
  args: { block: systemBlocks.planContent },
}

export const AgentName: Story = {
  args: { block: systemBlocks.agentName },
}

// ── New system variants (zero-data-loss) ────────────────────────────────────

export const SessionInit: Story = {
  args: { block: systemBlocks.sessionInit },
}

export const SessionStatus: Story = {
  args: { block: systemBlocks.sessionStatus },
}

export const ElicitationComplete: Story = {
  args: { block: systemBlocks.elicitationComplete },
}

export const HookEvent: Story = {
  args: { block: systemBlocks.hookEvent },
}

export const HookEventError: Story = {
  args: { block: systemBlocks.hookEventError },
}

export const FilesSaved: Story = {
  args: { block: systemBlocks.filesSaved },
}

export const CommandOutput: Story = {
  args: { block: systemBlocks.commandOutput },
}

export const StreamDelta: Story = {
  args: { block: systemBlocks.streamDelta },
}

export const LocalCommand: Story = {
  args: { block: systemBlocks.localCommand },
}

export const FileHistorySnapshot: Story = {
  args: { block: systemBlocks.fileHistorySnapshot },
}

export const AiTitle: Story = {
  args: { block: systemBlocks.aiTitle },
}

export const LastPrompt: Story = {
  args: { block: systemBlocks.lastPrompt },
}

export const WorktreeState: Story = {
  args: { block: systemBlocks.worktreeState },
}

export const Informational: Story = {
  args: { block: systemBlocks.informational },
}

export const AttachmentHook: Story = {
  args: { block: systemBlocks.attachmentHook },
}

export const AttachmentFile: Story = {
  args: { block: systemBlocks.attachmentFile },
}

export const PermissionModeChange: Story = {
  args: { block: systemBlocks.permissionModeChange },
}

export const ScheduledTaskFire: Story = {
  args: { block: systemBlocks.scheduledTaskFire },
}

export const UnknownVariant: Story = {
  args: { block: systemBlocks.unknownVariant },
}
