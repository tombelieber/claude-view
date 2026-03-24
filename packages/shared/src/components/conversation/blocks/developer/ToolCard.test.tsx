import type { ToolExecution } from '../../../../types/blocks'
import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { ToolCard } from './ToolCard'

function makeExecution(overrides: Partial<ToolExecution> = {}): ToolExecution {
  return {
    toolName: 'Read',
    toolInput: { file_path: '/src/index.ts' },
    toolUseId: 'tu-abc123',
    status: 'complete',
    ...overrides,
  }
}

describe('ToolCard', () => {
  it('renders tool name and status icon', () => {
    render(<ToolCard execution={makeExecution()} />)
    expect(screen.getByText('Read')).toBeInTheDocument()
    // Complete status should show a check icon (via aria or test id)
    expect(screen.getByTestId('status-complete')).toBeInTheDocument()
  })

  it('shows category badge when category present', () => {
    render(<ToolCard execution={makeExecution({ category: 'mcp' })} />)
    expect(screen.getByText('mcp')).toBeInTheDocument()
  })

  it('shows duration badge when duration present', () => {
    render(<ToolCard execution={makeExecution({ duration: 1500 })} />)
    expect(screen.getByText('1.5s')).toBeInTheDocument()
  })

  it('collapses and re-expands input/output on click', async () => {
    const user = userEvent.setup()
    render(
      <ToolCard
        execution={makeExecution({
          toolName: 'CustomTool',
          toolInput: { command: 'echo hello' },
          result: { output: 'hello\n', isError: false, isReplay: false },
        })}
      />,
    )
    // Body should be visible initially (expanded by default)
    expect(screen.getByText('Input')).toBeInTheDocument()
    expect(screen.getByText('Output')).toBeInTheDocument()

    // Click the header row to collapse
    await user.click(screen.getByTestId('status-complete'))
    expect(screen.queryByText('Input')).not.toBeInTheDocument()

    // Click again to re-expand
    await user.click(screen.getByTestId('status-complete'))
    expect(screen.getByText('Input')).toBeInTheDocument()
  })

  it('shows error styling for error status', () => {
    const { container } = render(
      <ToolCard
        execution={makeExecution({
          status: 'error',
          result: { output: 'Command failed', isError: true, isReplay: false },
        })}
      />,
    )
    // The card wrapper should have red border
    const card = container.firstChild as HTMLElement
    expect(card.className).toContain('border-red')
  })

  it('shows progress timer when running', () => {
    render(
      <ToolCard
        execution={makeExecution({
          status: 'running',
          progress: { elapsedSeconds: 3.2 },
        })}
      />,
    )
    expect(screen.getByText('3.2s')).toBeInTheDocument()
    expect(screen.getByTestId('status-running')).toBeInTheDocument()
  })
})
