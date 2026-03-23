import type { Meta, StoryObj } from '@storybook/react-vite'
import { askQuestions } from '../../../../stories/fixtures'
import { AskUserQuestionCard } from './AskUserQuestionCard'

const meta = {
  title: 'Chat/Cards/AskUserQuestionCard',
  component: AskUserQuestionCard,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AskUserQuestionCard>

export default meta
type Story = StoryObj<typeof meta>

export const SingleSelect: Story = {
  args: {
    question: askQuestions.singleSelect,
    onAnswer: () => {},
  },
}

export const MultiSelect: Story = {
  args: {
    question: askQuestions.multiSelect,
    onAnswer: () => {},
  },
}

export const Answered: Story = {
  args: {
    question: askQuestions.singleSelect,
    answered: true,
    selectedAnswers: {
      'Which authentication strategy should I use for this service?': 'JWT with RSA-256',
    },
  },
}
