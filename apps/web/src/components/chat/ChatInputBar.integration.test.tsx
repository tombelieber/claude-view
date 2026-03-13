import type { ModelOption } from '@/hooks/use-models'
import type { SessionCapabilities } from '@/hooks/use-session-capabilities'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { createElement } from 'react'
import type { ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { ChatInputBar } from './ChatInputBar'

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  return ({ children }: { children: ReactNode }) =>
    createElement(QueryClientProvider, { client }, children)
}

function renderWithQuery(ui: React.ReactElement) {
  const Wrapper = createWrapper()
  return render(<Wrapper>{ui}</Wrapper>)
}

// No useModelOptions mock needed — ChatInputBar receives modelOptions as a prop
const mockModelOptions: ModelOption[] = [
  { id: 'claude-opus-4-6', label: 'Claude Opus 4.6' },
  { id: 'claude-sonnet-4-6', label: 'Claude Sonnet 4.6' },
]

const defaultCapabilities: SessionCapabilities = {
  model: 'claude-sonnet-4-6',
  permissionMode: 'default',
  slashCommands: ['commit', 'test'],
  mcpServers: [{ name: 'gh', status: 'connected' }],
}

describe('ChatInputBar + ChatPalette integration', () => {
  it('typing "/" opens command palette when capabilities provided', async () => {
    const user = userEvent.setup()
    renderWithQuery(
      <ChatInputBar
        onSend={vi.fn()}
        capabilities={defaultCapabilities}
        modelOptions={mockModelOptions}
      />,
    )
    const input = screen.getByTestId('chat-input')
    await user.type(input, '/')
    expect(screen.getByText('Context')).toBeTruthy()
  })

  it('selecting a slash command calls onCommand', async () => {
    const onCommand = vi.fn()
    const user = userEvent.setup()
    renderWithQuery(
      <ChatInputBar
        onSend={vi.fn()}
        onCommand={onCommand}
        capabilities={defaultCapabilities}
        modelOptions={mockModelOptions}
      />,
    )
    const input = screen.getByTestId('chat-input')
    await user.type(input, '/')
    await user.click(screen.getByText('/commit'))
    expect(onCommand).toHaveBeenCalledWith('commit')
  })

  it('selecting model switch calls onModelSwitch', async () => {
    const onModelSwitch = vi.fn()
    const user = userEvent.setup()
    renderWithQuery(
      <ChatInputBar
        onSend={vi.fn()}
        onModelSwitch={onModelSwitch}
        capabilities={defaultCapabilities}
        modelOptions={mockModelOptions}
      />,
    )
    const input = screen.getByTestId('chat-input')
    await user.type(input, '/')
    await user.click(screen.getByText('Switch model...'))
    await user.click(screen.getByText('Claude Opus 4.6'))
    expect(onModelSwitch).toHaveBeenCalledWith('claude-opus-4-6')
  })

  it('selecting current model is a noop', async () => {
    const onModelSwitch = vi.fn()
    const user = userEvent.setup()
    renderWithQuery(
      <ChatInputBar
        onSend={vi.fn()}
        onModelSwitch={onModelSwitch}
        capabilities={defaultCapabilities}
        modelOptions={mockModelOptions}
      />,
    )
    const input = screen.getByTestId('chat-input')
    await user.type(input, '/')
    await user.click(screen.getByText('Switch model...'))
    // "Claude Sonnet 4.6" appears in both ModelSelector chip and submenu
    const palette = screen.getByTestId('command-palette')
    // Find the button with "Claude Sonnet 4.6" text inside the palette
    const allButtons = palette.querySelectorAll('button')
    const sonnetBtn = Array.from(allButtons).find((b) =>
      b.textContent?.includes('Claude Sonnet 4.6'),
    )
    expect(sonnetBtn).toBeDefined()
    await user.click(sonnetBtn as HTMLElement)
    expect(onModelSwitch).not.toHaveBeenCalled()
  })

  it('palette closes after action selection', async () => {
    const user = userEvent.setup()
    renderWithQuery(
      <ChatInputBar
        onSend={vi.fn()}
        onCommand={vi.fn()}
        capabilities={defaultCapabilities}
        modelOptions={mockModelOptions}
      />,
    )
    const input = screen.getByTestId('chat-input')
    await user.type(input, '/')
    await user.click(screen.getByText('/commit'))
    expect(screen.queryByText('Context')).toBeNull()
  })

  it('renders without capabilities — graceful fallback to SlashCommandPopover', () => {
    renderWithQuery(<ChatInputBar onSend={vi.fn()} />)
    expect(screen.getByTestId('chat-input')).toBeTruthy()
  })
})
