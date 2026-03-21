import type { Meta, StoryObj } from '@storybook/react-vite'
import type { RichMessage } from '../RichPane'
import { ActionLogTab } from './ActionLogTab'

const NOW = Math.floor(Date.now() / 1000)

const messages: RichMessage[] = [
  { type: 'user', content: 'Refactor the auth middleware', ts: NOW - 300 },
  {
    type: 'tool_use',
    content: 'Read',
    name: 'Read',
    input: '{"file_path": "src/auth/middleware.rs"}',
    ts: NOW - 295,
    category: 'builtin',
  },
  {
    type: 'tool_result',
    content: 'fn validate_token(&self) { ... }',
    name: 'Read',
    ts: NOW - 294,
    category: 'builtin',
  },
  {
    type: 'tool_use',
    content: 'Edit',
    name: 'Edit',
    input:
      '{"file_path": "src/auth/middleware.rs", "old_string": "fn old()", "new_string": "fn new()"}',
    ts: NOW - 280,
    category: 'builtin',
  },
  {
    type: 'tool_result',
    content: 'Successfully edited',
    name: 'Edit',
    ts: NOW - 279,
    category: 'builtin',
  },
  {
    type: 'tool_use',
    content: 'Skill',
    name: 'Skill',
    input: '{"skill": "commit"}',
    ts: NOW - 260,
    category: 'skill',
  },
  {
    type: 'tool_use',
    content: 'mcp__postgres__query',
    name: 'mcp__postgres__query',
    input: '{"query": "SELECT 1"}',
    ts: NOW - 250,
    category: 'mcp',
  },
  {
    type: 'tool_use',
    content: 'Agent',
    name: 'Agent',
    input: '{"prompt": "Research patterns"}',
    ts: NOW - 240,
    category: 'agent',
  },
  {
    type: 'progress',
    content: 'hook_progress: pre-commit',
    ts: NOW - 230,
    category: 'hook',
    metadata: { type: 'hook_progress', hookName: 'pre-commit' },
  },
  {
    type: 'error',
    content: 'Rate limit exceeded',
    ts: NOW - 220,
    category: 'error',
    metadata: { retryInMs: 5000 },
  },
  {
    type: 'system',
    content: 'Turn completed in 45.5s',
    ts: NOW - 210,
    category: 'system',
    metadata: { type: 'turn_duration', durationMs: 45500 },
  },
  {
    type: 'system',
    content: 'file-history-snapshot',
    ts: NOW - 200,
    category: 'snapshot',
    metadata: { type: 'file-history-snapshot' },
  },
  {
    type: 'system',
    content: 'queue-enqueue',
    ts: NOW - 190,
    category: 'queue',
    metadata: { type: 'queue-operation' },
  },
  { type: 'assistant', content: 'Done! All tests pass.', ts: NOW - 180 },
]

function ActionLogTabWrapper(props: { messages: RichMessage[]; bufferDone: boolean }) {
  return (
    <div style={{ height: 500, width: '100%' }}>
      <ActionLogTab {...props} />
    </div>
  )
}

const meta = {
  title: 'Live/ActionLog/ActionLogTab',
  component: ActionLogTabWrapper,
  parameters: { layout: 'fullscreen', backgrounds: { default: 'dark' } },
} satisfies Meta<typeof ActionLogTabWrapper>

export default meta
type Story = StoryObj<typeof meta>

/** All action categories represented. */
export const AllCategories: Story = { args: { messages, bufferDone: true } }

/** Empty state. */
export const Empty: Story = { args: { messages: [], bufferDone: true } }

/** Loading state. */
export const Loading: Story = { args: { messages: messages.slice(0, 3), bufferDone: false } }
