import type { Meta, StoryObj } from '@storybook/react-vite'
import { InteractiveCardShell } from './InteractiveCardShell'

const meta = {
  title: 'Chat/Cards/InteractiveCardShell',
  component: InteractiveCardShell,
  parameters: { layout: 'padded' },
  decorators: [
    (Story) => (
      <div className="w-[480px] max-w-full">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof InteractiveCardShell>

export default meta
type Story = StoryObj<typeof meta>

export const Permission: Story = {
  args: {
    variant: 'permission',
    header: 'Permission Required',
    children: <p className="text-xs text-gray-700 dark:text-gray-300">Allow Bash: cargo test?</p>,
    actions: (
      <>
        <button
          type="button"
          className="px-3 py-1.5 text-xs font-medium text-red-700 bg-red-50 border border-red-200 rounded-md"
        >
          Deny
        </button>
        <button
          type="button"
          className="px-3 py-1.5 text-xs font-medium text-white bg-green-600 rounded-md"
        >
          Allow
        </button>
      </>
    ),
  },
}

export const Question: Story = {
  args: {
    variant: 'question',
    header: 'Question',
    children: (
      <p className="text-xs text-gray-700 dark:text-gray-300">Which approach do you prefer?</p>
    ),
    actions: (
      <button
        type="button"
        className="px-3 py-1.5 text-xs font-medium text-white bg-purple-600 rounded-md"
      >
        Submit
      </button>
    ),
  },
}

export const Plan: Story = {
  args: {
    variant: 'plan',
    header: 'Plan Approval',
    children: (
      <p className="text-xs text-gray-700 dark:text-gray-300">
        Step 1: Read file. Step 2: Edit file.
      </p>
    ),
    actions: (
      <>
        <button
          type="button"
          className="px-3 py-1.5 text-xs font-medium text-gray-700 bg-gray-100 border border-gray-200 rounded-md"
        >
          Request Changes
        </button>
        <button
          type="button"
          className="px-3 py-1.5 text-xs font-medium text-white bg-blue-600 rounded-md"
        >
          Approve Plan
        </button>
      </>
    ),
  },
}

export const Elicitation: Story = {
  args: {
    variant: 'elicitation',
    header: 'Input Requested',
    children: (
      <p className="text-xs text-gray-700 dark:text-gray-300">Enter the connection string:</p>
    ),
  },
}

export const ResolvedSuccess: Story = {
  args: {
    variant: 'permission',
    header: 'Permission Required',
    children: <p className="text-xs text-gray-700 dark:text-gray-300">Allow Bash: cargo test?</p>,
    resolved: { label: 'Allowed', variant: 'success' },
  },
}

export const ResolvedDenied: Story = {
  args: {
    variant: 'permission',
    header: 'Permission Required',
    children: <p className="text-xs text-gray-700 dark:text-gray-300">Allow Bash: rm -rf /?</p>,
    resolved: { label: 'Denied', variant: 'denied' },
  },
}

export const ResolvedNeutral: Story = {
  args: {
    variant: 'elicitation',
    header: 'Input Requested',
    children: (
      <p className="text-xs text-gray-700 dark:text-gray-300">Connection string provided.</p>
    ),
    resolved: { label: 'Submitted', variant: 'neutral' },
  },
}
