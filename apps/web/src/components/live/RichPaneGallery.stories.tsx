/**
 * Gallery story — render ALL RichMessage types + ALL ActionRow categories in one scrollable view.
 */
import type { Meta, StoryObj } from '@storybook/react-vite'
import type { RichMessage } from './RichPane'
import { RichPane } from './RichPane'
import { ActionRow } from './action-log/ActionRow'
import { TurnSeparatorRow } from './action-log/TurnSeparatorRow'
import type { ActionItem } from './action-log/types'

const NOW = Math.floor(Date.now() / 1000)

// ── ALL RichMessage types ───────────────────────────────────────────────────

const allRichMessages: RichMessage[] = [
  { type: 'user', content: 'Refactor the auth middleware', ts: NOW - 300 },
  { type: 'user', content: 'Run the tests', ts: NOW - 298, pending: true },
  {
    type: 'thinking',
    content: 'Let me analyze the current middleware structure and identify coupling issues...',
    ts: NOW - 295,
  },
  {
    type: 'assistant',
    content:
      "I'll refactor the middleware. Here's the plan:\n\n1. Extract TokenValidator\n2. Move session logic\n3. Update tests",
    ts: NOW - 290,
  },
  {
    type: 'tool_use',
    content: 'Read',
    name: 'Read',
    input: '{"file_path": "src/auth/middleware.rs"}',
    inputData: { file_path: 'src/auth/middleware.rs' },
    ts: NOW - 285,
    category: 'builtin',
  },
  {
    type: 'tool_result',
    content: 'pub fn validate_token(&self, token: &str) -> Result<Claims> { /* 450 lines */ }',
    name: 'Read',
    ts: NOW - 284,
    category: 'builtin',
  },
  {
    type: 'tool_use',
    content: 'Edit',
    name: 'Edit',
    input:
      '{"file_path": "src/auth/middleware.rs", "old_string": "fn old()", "new_string": "fn new()"}',
    inputData: {
      file_path: 'src/auth/middleware.rs',
      old_string: 'fn old()',
      new_string: 'fn new()',
    },
    ts: NOW - 280,
    category: 'builtin',
  },
  {
    type: 'tool_use',
    content: 'Skill',
    name: 'Skill',
    input: '{"skill": "commit"}',
    inputData: { skill: 'commit' },
    ts: NOW - 270,
    category: 'skill',
  },
  {
    type: 'tool_use',
    content: 'mcp__postgres__query',
    name: 'mcp__postgres__query',
    input: '{"query": "SELECT 1"}',
    inputData: { query: 'SELECT 1' },
    ts: NOW - 260,
    category: 'mcp',
  },
  {
    type: 'tool_use',
    content: 'Agent',
    name: 'Agent',
    input: '{"prompt": "Research auth patterns"}',
    inputData: { prompt: 'Research auth patterns' },
    ts: NOW - 250,
    category: 'agent',
  },
  {
    type: 'hook',
    content: 'PreToolUse:Bash — live-monitor: validating',
    ts: NOW - 240,
    category: 'hook',
    metadata: { type: 'hook_progress', hookName: 'live-monitor', hookEvent: 'PreToolUse' },
  },
  {
    type: 'progress',
    content: 'bash_progress: compiling',
    ts: NOW - 235,
    category: 'builtin',
    metadata: {
      type: 'bash_progress',
      output: 'Compiling...',
      elapsedTimeSeconds: 5.2,
      totalLines: 10,
    },
  },
  {
    type: 'progress',
    content: 'agent_progress: researching',
    ts: NOW - 230,
    category: 'agent',
    metadata: { type: 'agent_progress', agentId: 'agent_001', prompt: 'Research auth' },
  },
  {
    type: 'progress',
    content: 'hook_progress: pre-commit',
    ts: NOW - 225,
    category: 'hook',
    metadata: { type: 'hook_progress', hookName: 'pre-commit', hookEvent: 'PreToolUse' },
  },
  {
    type: 'progress',
    content: 'mcp_progress: postgres/query',
    ts: NOW - 220,
    category: 'mcp',
    metadata: {
      type: 'mcp_progress',
      serverName: 'postgres',
      toolName: 'query',
      status: 'running',
    },
  },
  {
    type: 'system',
    content: 'Turn completed in 45.5s',
    ts: NOW - 215,
    metadata: { type: 'turn_duration', durationMs: 45500 },
  },
  {
    type: 'system',
    content: 'queue-enqueue: fix bug',
    ts: NOW - 210,
    category: 'queue',
    metadata: { type: 'queue-operation', operation: 'enqueue' },
  },
  {
    type: 'system',
    content: 'file-history-snapshot',
    ts: NOW - 205,
    category: 'snapshot',
    metadata: { type: 'file-history-snapshot' },
  },
  {
    type: 'system',
    content: 'Context compacted (auto, 145K)',
    ts: NOW - 200,
    category: 'system',
    metadata: { type: 'compact_boundary', trigger: 'auto', preTokens: 145000 },
  },
  {
    type: 'error',
    content: 'Rate limit exceeded. Retry in 5s.',
    ts: NOW - 195,
    category: 'error',
    metadata: { retryInMs: 5000 },
  },
  {
    type: 'error',
    content: 'API error: 500 Internal Server Error',
    ts: NOW - 190,
    category: 'error',
  },
  {
    type: 'assistant',
    content:
      'Done! All refactoring complete. Tests pass:\n\n```\ntest result: ok. 42 passed; 0 failed\n```',
    ts: NOW - 180,
  },
]

// ── ALL ActionRow categories ────────────────────────────────────────────────

const allActions: ActionItem[] = [
  {
    id: 'ar-1',
    timestamp: NOW,
    category: 'builtin',
    toolName: 'Bash',
    label: 'cargo test',
    status: 'success',
    duration: 12500,
    input: '{"command":"cargo test"}',
    output: 'ok. 42 passed',
  },
  {
    id: 'ar-2',
    timestamp: NOW,
    category: 'builtin',
    toolName: 'Read',
    label: 'src/main.rs',
    status: 'success',
    duration: 50,
  },
  {
    id: 'ar-3',
    timestamp: NOW,
    category: 'builtin',
    toolName: 'Bash',
    label: 'rm /nonexistent',
    status: 'error',
    duration: 100,
    output: 'No such file',
  },
  {
    id: 'ar-4',
    timestamp: NOW,
    category: 'builtin',
    toolName: 'Bash',
    label: 'cargo build --release',
    status: 'pending',
  },
  {
    id: 'ar-5',
    timestamp: NOW,
    category: 'skill',
    toolName: 'Skill',
    label: 'commit',
    status: 'success',
    duration: 3000,
  },
  {
    id: 'ar-6',
    timestamp: NOW,
    category: 'mcp',
    toolName: 'mcp__postgres__query',
    label: 'SELECT * FROM sessions',
    status: 'success',
    duration: 200,
  },
  {
    id: 'ar-7',
    timestamp: NOW,
    category: 'agent',
    toolName: 'Agent',
    label: 'Research auth patterns',
    status: 'success',
    duration: 25000,
  },
  {
    id: 'ar-8',
    timestamp: NOW,
    category: 'hook',
    toolName: 'pre-commit',
    label: 'PreToolUse:Bash — live-monitor',
    status: 'success',
    duration: 80,
  },
  {
    id: 'ar-9',
    timestamp: NOW,
    category: 'hook_progress',
    toolName: 'hook',
    label: 'hook running...',
    status: 'pending',
  },
  {
    id: 'ar-10',
    timestamp: NOW,
    category: 'error',
    toolName: 'API',
    label: 'Rate limit exceeded',
    status: 'error',
    output: '429',
  },
  {
    id: 'ar-11',
    timestamp: NOW,
    category: 'system',
    toolName: 'system',
    label: 'turn_duration: 45.5s',
    status: 'success',
    duration: 45500,
  },
  {
    id: 'ar-12',
    timestamp: NOW,
    category: 'snapshot',
    toolName: 'file-history',
    label: '12 files changed',
    status: 'success',
  },
  {
    id: 'ar-13',
    timestamp: NOW,
    category: 'queue',
    toolName: 'queue',
    label: 'enqueue: fix the login bug',
    status: 'success',
  },
]

// ── Gallery layout ──────────────────────────────────────────────────────────

function LiveGallery() {
  return (
    <div className="min-h-screen bg-gray-950 text-gray-100">
      {/* RichPane — all message types */}
      <section className="border-b border-gray-800">
        <h2 className="px-4 py-2 text-sm font-bold uppercase tracking-wider text-gray-400 bg-gray-900">
          RichPane — All 9 message types ({allRichMessages.length} messages, verbose mode)
        </h2>
        <div style={{ height: 500 }}>
          <RichPane
            messages={allRichMessages}
            isVisible={true}
            verboseMode={true}
            bufferDone={true}
          />
        </div>
      </section>

      {/* RichPane — chat only */}
      <section className="border-b border-gray-800">
        <h2 className="px-4 py-2 text-sm font-bold uppercase tracking-wider text-gray-400 bg-gray-900">
          RichPane — Chat-only mode (verboseMode=false)
        </h2>
        <div style={{ height: 300 }}>
          <RichPane
            messages={allRichMessages}
            isVisible={true}
            verboseMode={false}
            bufferDone={true}
          />
        </div>
      </section>

      {/* ActionRow — all categories */}
      <section className="border-b border-gray-800">
        <h2 className="px-4 py-2 text-sm font-bold uppercase tracking-wider text-gray-400 bg-gray-900">
          ActionRow — All 10 categories + 3 statuses ({allActions.length} rows)
        </h2>
        <div className="divide-y divide-gray-800/50">
          {allActions.map((action) => (
            <ActionRow key={action.id} action={action} />
          ))}
        </div>
      </section>

      {/* TurnSeparatorRow — both roles */}
      <section>
        <h2 className="px-4 py-2 text-sm font-bold uppercase tracking-wider text-gray-400 bg-gray-900">
          TurnSeparatorRow — Both roles
        </h2>
        {/* biome-ignore lint/a11y/useValidAriaRole: role is a component prop, not HTML aria role */}
        <TurnSeparatorRow role="user" content="Can you help me refactor the auth middleware?" />
        {/* biome-ignore lint/a11y/useValidAriaRole: role is a component prop, not HTML aria role */}
        <TurnSeparatorRow role="assistant" content="I'll start by reading the implementation..." />
      </section>
    </div>
  )
}

const meta = {
  title: 'Gallery/LiveComponents',
  component: LiveGallery,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof LiveGallery>

export default meta
type Story = StoryObj<typeof meta>

/** ALL RichPane message types + ALL ActionRow categories in one view. */
export const AllComponents: Story = {}
