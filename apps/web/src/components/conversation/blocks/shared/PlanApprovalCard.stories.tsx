import type { Meta, StoryObj } from '@storybook/react-vite'
import { planApprovals } from '../../../../stories/fixtures'
import { PlanApprovalCard } from './PlanApprovalCard'

const meta = {
  title: 'Chat/Cards/PlanApprovalCard',
  component: PlanApprovalCard,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof PlanApprovalCard>

export default meta
type Story = StoryObj<typeof meta>

export const Pending: Story = {
  args: {
    plan: planApprovals.simple,
    onApprove: () => {},
  },
}

export const Approved: Story = {
  args: {
    plan: planApprovals.simple,
    resolved: { approved: true },
  },
}

export const Rejected: Story = {
  args: {
    plan: planApprovals.simple,
    resolved: { approved: false },
  },
}
