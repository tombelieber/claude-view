import type { Meta, StoryObj } from '@storybook/react-vite'
import type { RichMessage, RichPaneProps } from './RichPane'
import { RichPane } from './RichPane'

const NOW = Math.floor(Date.now() / 1000)

// ── RichMessage fixtures (all 9 message types) ──────────────────────────────

const richMessages: RichMessage[] = [
  // user
  {
    type: 'user',
    content: 'Can you help me refactor the authentication middleware?',
    ts: NOW - 300,
  },
  // assistant (text)
  {
    type: 'assistant',
    content:
      "I'll help you refactor the auth middleware. Let me start by reading the current implementation.\n\n```rust\npub struct AuthMiddleware {\n    jwks: JwkSet,\n}\n```",
    ts: NOW - 295,
  },
  // thinking
  {
    type: 'thinking',
    content:
      'The user wants to refactor auth middleware. I should examine the current structure and suggest improvements based on separation of concerns...',
    ts: NOW - 294,
  },
  // tool_use
  {
    type: 'tool_use',
    content: 'Read',
    name: 'Read',
    input: '{"file_path": "/Users/dev/project/src/auth/middleware.rs"}',
    inputData: { file_path: '/Users/dev/project/src/auth/middleware.rs' },
    ts: NOW - 290,
    category: 'builtin',
  },
  // tool_result
  {
    type: 'tool_result',
    content:
      'pub fn validate_token(&self, token: &str) -> Result<Claims> {\n    // ... 450 lines\n}',
    name: 'Read',
    ts: NOW - 289,
    category: 'builtin',
  },
  // tool_use (MCP)
  {
    type: 'tool_use',
    content: 'mcp__postgres__query',
    name: 'mcp__postgres__query',
    input: '{"query": "SELECT * FROM sessions LIMIT 5"}',
    inputData: { query: 'SELECT * FROM sessions LIMIT 5' },
    ts: NOW - 280,
    category: 'mcp',
  },
  // hook
  {
    type: 'hook',
    content: 'PreToolUse:Bash — live-monitor: validating tool use',
    ts: NOW - 275,
    category: 'hook',
    metadata: { type: 'hook_progress', hookName: 'live-monitor', hookEvent: 'PreToolUse' },
  },
  // progress (bash)
  {
    type: 'progress',
    content: 'bash_progress: Compiling claude-view-core v0.23.0',
    ts: NOW - 270,
    category: 'builtin',
    metadata: {
      type: 'bash_progress',
      output: 'Compiling claude-view-core v0.23.0\n   Compiling claude-view-server v0.23.0',
      elapsedTimeSeconds: 12.3,
      totalLines: 3,
    },
  },
  // progress (agent)
  {
    type: 'progress',
    content: 'agent_progress: Research authentication patterns',
    ts: NOW - 265,
    category: 'agent',
    metadata: {
      type: 'agent_progress',
      agentId: 'agent_research_001',
      prompt: 'Research authentication best practices',
    },
  },
  // progress (hook)
  {
    type: 'progress',
    content: 'hook_progress: pre-commit running',
    ts: NOW - 260,
    category: 'hook',
    metadata: {
      type: 'hook_progress',
      hookName: 'pre-commit',
      hookEvent: 'PreToolUse',
      command: '/Users/dev/.claude/hooks/pre-tool.sh',
    },
  },
  // progress (mcp)
  {
    type: 'progress',
    content: 'mcp_progress: postgres/query started',
    ts: NOW - 255,
    category: 'mcp',
    metadata: {
      type: 'mcp_progress',
      serverName: 'postgres',
      toolName: 'query',
      status: 'running',
    },
  },
  // system (turn_duration)
  {
    type: 'system',
    content: 'Turn completed in 12.5s',
    ts: NOW - 250,
    metadata: { type: 'turn_duration', durationMs: 12500 },
  },
  // system (queue-operation)
  {
    type: 'system',
    content: 'queue-enqueue: Fix the login bug',
    ts: NOW - 245,
    metadata: { type: 'queue-operation', operation: 'enqueue' },
    category: 'queue',
  },
  // system (file-history-snapshot)
  {
    type: 'system',
    content: 'file-history-snapshot',
    ts: NOW - 240,
    metadata: { type: 'file-history-snapshot' },
    category: 'snapshot',
  },
  // system (compact_boundary)
  {
    type: 'system',
    content: 'Context compacted (auto, 145K tokens)',
    ts: NOW - 235,
    metadata: { type: 'compact_boundary', trigger: 'auto', preTokens: 145000 },
    category: 'system',
  },
  // error
  {
    type: 'error',
    content: 'Rate limit exceeded. Retry in 5s.',
    ts: NOW - 230,
    category: 'error',
    metadata: { retryInMs: 5000, retryAttempt: 1 },
  },
  // user (pending/queued)
  {
    type: 'user',
    content: 'Run the tests',
    ts: NOW - 225,
    pending: true,
  },
  // tool_use (Skill)
  {
    type: 'tool_use',
    content: 'Skill',
    name: 'Skill',
    input: '{"skill": "commit"}',
    inputData: { skill: 'commit' },
    ts: NOW - 220,
    category: 'skill',
  },
  // tool_use (Agent)
  {
    type: 'tool_use',
    content: 'Agent',
    name: 'Agent',
    input: '{"prompt": "Research auth patterns"}',
    inputData: { prompt: 'Research auth patterns' },
    ts: NOW - 215,
    category: 'agent',
  },
  // assistant (final response)
  {
    type: 'assistant',
    content:
      "Here's the refactored middleware:\n\n| Module | Before | After |\n|--------|--------|-------|\n| `auth/middleware.rs` | 450 lines | 120 lines |\n| `auth/validator.rs` | — | 180 lines |\n\nAll tests pass. Ready to commit.",
    ts: NOW - 200,
  },
]

// ── Storybook setup ─────────────────────────────────────────────────────────

function RichPaneWrapper(props: RichPaneProps) {
  return (
    <div style={{ height: 600, width: '100%' }}>
      <RichPane {...props} />
    </div>
  )
}

const meta = {
  title: 'Live/RichPane',
  component: RichPaneWrapper,
  parameters: {
    layout: 'fullscreen',
    backgrounds: { default: 'dark' },
  },
} satisfies Meta<typeof RichPaneWrapper>

export default meta
type Story = StoryObj<typeof meta>

/** All 9 RichMessage types in verbose mode — full gallery. */
export const AllMessageTypes: Story = {
  args: {
    messages: richMessages,
    isVisible: true,
    verboseMode: true,
    bufferDone: true,
  },
}

/** Chat-only mode (verboseMode=false) — only user + assistant + error. */
export const ChatOnly: Story = {
  args: {
    messages: richMessages,
    isVisible: true,
    verboseMode: false,
    bufferDone: true,
  },
}

/** Empty state. */
export const Empty: Story = {
  args: {
    messages: [],
    isVisible: true,
    verboseMode: true,
    bufferDone: true,
  },
}

/** Loading state (bufferDone=false). */
export const Loading: Story = {
  args: {
    messages: richMessages.slice(0, 3),
    isVisible: true,
    verboseMode: true,
    bufferDone: false,
  },
}

/** Only errors. */
export const ErrorsOnly: Story = {
  args: {
    messages: [
      {
        type: 'error',
        content: 'Rate limit exceeded',
        ts: NOW - 100,
        category: 'error',
        metadata: { retryInMs: 5000 },
      },
      {
        type: 'error',
        content: 'API error: 500 Internal Server Error',
        ts: NOW - 50,
        category: 'error',
      },
    ],
    isVisible: true,
    verboseMode: true,
    bufferDone: true,
  },
}

/** With rawJson (verbose debug mode). */
export const WithRawJson: Story = {
  args: {
    messages: [
      {
        type: 'user',
        content: 'Hello',
        ts: NOW - 100,
        rawJson: {
          type: 'user',
          uuid: 'u-1',
          message: { content: [{ type: 'text', text: 'Hello' }] },
          timestamp: '2026-03-21T01:00:00.000Z',
        },
      },
      {
        type: 'assistant',
        content: 'Hi there!',
        ts: NOW - 95,
        rawJson: {
          type: 'assistant',
          uuid: 'a-1',
          message: {
            id: 'msg-001',
            model: 'claude-sonnet-4-6',
            content: [{ type: 'text', text: 'Hi there!' }],
          },
        },
      },
    ],
    isVisible: true,
    verboseMode: true,
    bufferDone: true,
  },
}
