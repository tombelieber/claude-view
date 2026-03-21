import type { Meta, StoryObj } from '@storybook/react-vite'
import { ActionRow } from './ActionRow'
import type { ActionItem } from './types'

const meta = {
  title: 'Live/ActionLog/ActionRow',
  component: ActionRow,
  parameters: { layout: 'padded', backgrounds: { default: 'dark' } },
} satisfies Meta<typeof ActionRow>

export default meta
type Story = StoryObj<typeof meta>

const NOW = Math.floor(Date.now() / 1000)

const base: ActionItem = {
  id: 'a-1',
  timestamp: NOW,
  category: 'builtin',
  toolName: 'Bash',
  label: 'cargo test --workspace',
  status: 'success',
  duration: 12500,
  input: '{"command": "cargo test --workspace"}',
  output: 'test result: ok. 42 passed; 0 failed',
}

export const BuiltinSuccess: Story = { args: { action: base } }

export const BuiltinPending: Story = {
  args: {
    action: { ...base, id: 'a-2', status: 'pending', duration: undefined, output: undefined },
  },
}

export const BuiltinError: Story = {
  args: {
    action: {
      ...base,
      id: 'a-3',
      status: 'error',
      toolName: 'Bash',
      label: 'rm -rf /nonexistent',
      output: 'rm: /nonexistent: No such file or directory',
      duration: 150,
    },
  },
}

export const SkillAction: Story = {
  args: {
    action: { ...base, id: 'a-4', category: 'skill', toolName: 'Skill', label: 'commit' },
  },
}

export const McpAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-5',
      category: 'mcp',
      toolName: 'mcp__postgres__query',
      label: 'SELECT * FROM sessions LIMIT 10',
    },
  },
}

export const AgentAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-6',
      category: 'agent',
      toolName: 'Agent',
      label: 'Research authentication patterns',
      duration: 25000,
    },
  },
}

export const HookAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-7',
      category: 'hook',
      toolName: 'pre-commit',
      label: 'PreToolUse:Bash — live-monitor',
      duration: 80,
    },
  },
}

export const ErrorAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-8',
      category: 'error',
      toolName: 'API',
      label: 'Rate limit exceeded',
      status: 'error',
      output: '429 Too Many Requests',
    },
  },
}

export const SystemAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-9',
      category: 'system',
      toolName: 'system',
      label: 'turn_duration: 45.5s',
      duration: 45500,
    },
  },
}

export const SnapshotAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-10',
      category: 'snapshot',
      toolName: 'file-history',
      label: 'file-history-snapshot: 12 files',
    },
  },
}

export const QueueAction: Story = {
  args: {
    action: {
      ...base,
      id: 'a-11',
      category: 'queue',
      toolName: 'queue',
      label: 'enqueue: fix the login bug',
    },
  },
}

export const LongDuration: Story = {
  args: {
    action: { ...base, id: 'a-12', duration: 180000, label: 'cargo build --release' },
  },
}

export const NoInputOutput: Story = {
  args: {
    action: { ...base, id: 'a-13', input: undefined, output: undefined, label: 'system event' },
  },
}
