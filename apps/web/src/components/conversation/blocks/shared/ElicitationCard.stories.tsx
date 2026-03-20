import type { Meta, StoryObj } from '@storybook/react-vite'
import { elicitations } from '../../../../stories/fixtures'
import { ElicitationCard } from './ElicitationCard'

const meta = {
  title: 'Chat/Cards/ElicitationCard',
  component: ElicitationCard,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ElicitationCard>

export default meta
type Story = StoryObj<typeof meta>

export const Pending: Story = {
  args: {
    elicitation: elicitations.simple,
    onSubmit: () => {},
  },
}

export const Submitted: Story = {
  args: {
    elicitation: elicitations.simple,
    resolved: true,
  },
}
