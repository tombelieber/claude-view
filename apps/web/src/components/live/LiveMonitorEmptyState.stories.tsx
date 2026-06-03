import type { Meta, StoryObj } from '@storybook/react-vite'
import { MemoryRouter } from 'react-router-dom'
import { LiveMonitorEmptyState } from './LiveMonitorEmptyState'

/**
 * Live Monitor first-run / no-active-sessions state.
 *
 * Shown when a user opens claude-view with no *active* Claude Code session —
 * the common first-run case. Replaces the previous blank board on the default
 * kanban view and offers a path into indexed history.
 */
const meta = {
  title: 'Live/LiveMonitorEmptyState',
  component: LiveMonitorEmptyState,
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <MemoryRouter>
        <div className="dark min-h-[420px] bg-gray-950 flex items-center justify-center">
          <Story />
        </div>
      </MemoryRouter>
    ),
  ],
} satisfies Meta<typeof LiveMonitorEmptyState>

export default meta
type Story = StoryObj<typeof meta>

/** Nothing running, nothing detected — pure first-run. */
export const NoSessions: Story = {
  args: { processCount: 0 },
}

/** A Claude Code process is running but hasn't reported a session yet. */
export const ProcessDetected: Story = {
  args: { processCount: 1 },
}

/** Several processes detected, waiting on hook reports. */
export const MultipleProcesses: Story = {
  args: { processCount: 3 },
}
