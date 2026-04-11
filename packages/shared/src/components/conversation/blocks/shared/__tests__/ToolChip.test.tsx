import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ToolChip } from '../ToolChip'
import type { ToolExecution } from '../../../../../types/blocks'

function makeExecution(overrides: Partial<ToolExecution> = {}): ToolExecution {
  return {
    toolName: 'Bash',
    toolInput: { command: 'echo hello' },
    toolUseId: 'tu-1',
    status: 'complete',
    ...overrides,
  }
}

describe('ToolChip', () => {
  it('renders tool name', () => {
    render(<ToolChip execution={makeExecution()} />)
    expect(screen.getByText('Bash')).toBeInTheDocument()
  })

  it('renders preview text from toolInput.command', () => {
    render(<ToolChip execution={makeExecution({ toolInput: { command: 'ls -la' } })} />)
    expect(screen.getByText('ls -la')).toBeInTheDocument()
  })

  it('renders parentToolUseId indicator when present', () => {
    render(<ToolChip execution={makeExecution({ parentToolUseId: 'parent-abc-1234567' })} />)
    // Renders the first 8 chars of parentToolUseId
    expect(screen.getByText(/parent-a/)).toBeInTheDocument()
  })

  it('does not render parentToolUseId indicator when absent', () => {
    render(<ToolChip execution={makeExecution({ parentToolUseId: undefined })} />)
    expect(screen.queryByText(/->/)).not.toBeInTheDocument()
  })

  it('renders elapsedSeconds while running', () => {
    render(
      <ToolChip
        execution={makeExecution({
          status: 'running',
          progress: { elapsedSeconds: 3.7 },
        })}
      />,
    )
    expect(screen.getByText('3.7s')).toBeInTheDocument()
  })

  it('does not render elapsed seconds when status is complete', () => {
    render(
      <ToolChip
        execution={makeExecution({
          status: 'complete',
          progress: { elapsedSeconds: 3.7 },
        })}
      />,
    )
    expect(screen.queryByText('3.7s')).not.toBeInTheDocument()
  })

  it('applies red tint on result hint when result.isError is true', () => {
    const { container } = render(
      <ToolChip
        execution={makeExecution({
          toolName: 'Read',
          toolInput: { file_path: 'foo.ts' },
          status: 'complete',
          result: { output: 'line1\nline2\nline3', isError: true, isReplay: false },
        })}
      />,
    )
    // The result hint span should contain text-red-500 class
    const redSpan = container.querySelector('.text-red-500, .dark\\:text-red-400')
    expect(redSpan).not.toBeNull()
  })

  it('renders without red tint when result.isError is false', () => {
    render(
      <ToolChip
        execution={makeExecution({
          toolName: 'Read',
          toolInput: { file_path: 'foo.ts' },
          status: 'complete',
          result: { output: 'line1\nline2\nline3', isError: false, isReplay: false },
        })}
      />,
    )
    expect(screen.getByText('3 lines')).toBeInTheDocument()
  })

  it('renders error reason line below chip when status is error', () => {
    render(
      <ToolChip
        execution={makeExecution({
          status: 'error',
          result: { output: 'Permission denied\nmore info', isError: true, isReplay: false },
        })}
      />,
    )
    expect(screen.getByText('Permission denied')).toBeInTheDocument()
  })

  it('renders complete status icon (Check icon present)', () => {
    const { container } = render(<ToolChip execution={makeExecution({ status: 'complete' })} />)
    // Check icon rendered — look for SVG with green color class
    const svg = container.querySelector('svg.text-green-500, svg.dark\\:text-green-400')
    expect(svg).not.toBeNull()
  })

  it('renders MCP tool with teal Plug style', () => {
    const { container } = render(
      <ToolChip
        execution={makeExecution({
          toolName: 'mcp_custom_tool',
          toolInput: {},
          category: 'mcp',
          status: 'complete',
        })}
      />,
    )
    // Teal background applied
    const chip = container.querySelector('.bg-teal-50, .dark\\:bg-teal-900\\/20')
    expect(chip).not.toBeNull()
  })

  it('renders file path preview for Read tool', () => {
    render(
      <ToolChip
        execution={makeExecution({
          toolName: 'Read',
          toolInput: { file_path: '/some/path/to/myfile.ts' },
          status: 'complete',
          result: { output: 'content', isError: false, isReplay: false },
        })}
      />,
    )
    expect(screen.getByText('myfile.ts')).toBeInTheDocument()
  })

  it('shows duration label when duration is set and long enough', () => {
    render(
      <ToolChip
        execution={makeExecution({
          status: 'complete',
          duration: 2500,
        })}
      />,
    )
    expect(screen.getByText('2.5s')).toBeInTheDocument()
  })
})
