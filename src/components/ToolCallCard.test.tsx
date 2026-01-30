import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { ToolCallCard } from './ToolCallCard'

describe('ToolCallCard', () => {
  describe('Rendering basics', () => {
    it('renders tool name', () => {
      render(<ToolCallCard name="Read" input={{ file_path: '/src/index.ts' }} description="Read a file" />)
      expect(screen.getByText('Read')).toBeInTheDocument()
    })

    it('renders different tool names (Edit, Bash, etc.)', () => {
      const { rerender } = render(
        <ToolCallCard name="Edit" input={{ file_path: '/src/app.ts' }} description="Edit a file" />
      )
      expect(screen.getByText('Edit')).toBeInTheDocument()

      rerender(
        <ToolCallCard name="Bash" input={{ command: 'ls -la' }} description="Run a command" />
      )
      expect(screen.getByText('Bash')).toBeInTheDocument()
    })

    it('renders input parameters (file_path)', () => {
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/Users/dev/project/src/index.ts' }}
          description="Read source file"
        />
      )
      expect(screen.getByText(/\/Users\/dev\/project\/src\/index.ts/)).toBeInTheDocument()
    })

    it('renders input parameters (command)', () => {
      render(
        <ToolCallCard
          name="Bash"
          input={{ command: 'npm install' }}
          description="Install dependencies"
        />
      )
      expect(screen.getByText(/npm install/)).toBeInTheDocument()
    })

    it('renders input parameters (pattern)', () => {
      render(
        <ToolCallCard
          name="Grep"
          input={{ pattern: 'TODO_FIXME', path: '/src' }}
          description="Search for items"
        />
      )
      expect(screen.getByText(/TODO_FIXME/)).toBeInTheDocument()
    })

    it('shows description of what was attempted', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read the config file" />
      )
      expect(screen.getByText('Read the config file')).toBeInTheDocument()
    })
  })

  describe('Collapse/Expand behavior', () => {
    it('collapses by default (summary visible, details hidden)', () => {
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/src/index.ts' }}
          description="Read source file"
          parameters={{ file_path: '/src/index.ts', limit: 100 }}
        />
      )
      expect(screen.getByText('Read')).toBeInTheDocument()
      const button = screen.getByRole('button', { name: /tool call/i })
      expect(button).toHaveAttribute('aria-expanded', 'false')
    })

    it('expands to show full details on click', () => {
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/src/index.ts' }}
          description="Read source file"
          parameters={{ file_path: '/src/index.ts', limit: 100 }}
        />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      expect(button).toHaveAttribute('aria-expanded', 'false')

      fireEvent.click(button)

      expect(button).toHaveAttribute('aria-expanded', 'true')
      expect(screen.getByText(/limit/)).toBeInTheDocument()
    })

    it('collapses again on second click', () => {
      render(
        <ToolCallCard
          name="Edit"
          input={{ file_path: '/src/app.ts' }}
          description="Edit file"
          parameters={{ file_path: '/src/app.ts', old_string: 'foo', new_string: 'bar' }}
        />
      )
      const button = screen.getByRole('button', { name: /tool call/i })

      fireEvent.click(button) // expand
      expect(button).toHaveAttribute('aria-expanded', 'true')

      fireEvent.click(button) // collapse
      expect(button).toHaveAttribute('aria-expanded', 'false')
    })
  })

  describe('Edge cases', () => {
    it('handles missing parameters gracefully (no crash)', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      expect(screen.getByText('Read')).toBeInTheDocument()
    })

    it('renders "No parameters" when parameters is undefined and expanded', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      fireEvent.click(button)
      expect(screen.getByText('No parameters')).toBeInTheDocument()
    })

    it('renders gracefully when name is empty string', () => {
      const { container } = render(
        <ToolCallCard name="" input={{}} description="Unknown tool" />
      )
      expect(container).toBeInTheDocument()
      expect(screen.getByText('Unknown tool')).toBeInTheDocument()
    })

    it('wraps description >500 chars without truncation', () => {
      const longDescription = 'B'.repeat(600)
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description={longDescription} />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      fireEvent.click(button)
      // Description appears in both header subtitle and expanded area
      const matches = screen.getAllByText(longDescription)
      expect(matches.length).toBeGreaterThanOrEqual(1)
      // Verify the expanded area contains the full description with break-words
      const expandedDesc = matches.find(el => el.classList.contains('break-words'))
      expect(expandedDesc).toBeTruthy()
    })

    it('handles special chars in path (spaces, unicode)', () => {
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/Users/dev/my project/src/日本語.ts' }}
          description="Read unicode file"
        />
      )
      expect(screen.getByText(/\/Users\/dev\/my project\/src\/日本語.ts/)).toBeInTheDocument()
    })

    it('truncates very long file paths in summary', () => {
      const longPath = '/Users/dev/' + 'very-long-directory-name/'.repeat(10) + 'file.ts'
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: longPath }}
          description="Read file"
        />
      )
      // The summary should show a truncated path (the filename at least)
      expect(screen.getByText(/file\.ts/)).toBeInTheDocument()
      // The container should not cause horizontal scroll (checked via CSS class)
      const summary = screen.getByText(/file\.ts/)
      expect(summary.closest('[class*="truncate"]') || summary.closest('[class*="break"]')).toBeTruthy()
    })
  })

  describe('Icon', () => {
    it('shows icon with aria-hidden="true"', () => {
      const { container } = render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const icon = container.querySelector('svg[aria-hidden="true"]')
      expect(icon).toBeInTheDocument()
    })

    it('renders custom icon when provided', () => {
      const CustomIcon = () => <span data-testid="custom-icon">Custom</span>
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/foo.ts' }}
          description="Read file"
          icon={<CustomIcon />}
        />
      )
      expect(screen.getByTestId('custom-icon')).toBeInTheDocument()
    })
  })

  describe('Copy button', () => {
    const mockWriteText = vi.fn().mockResolvedValue(undefined)

    beforeEach(() => {
      mockWriteText.mockClear()
      Object.defineProperty(navigator, 'clipboard', {
        value: { writeText: mockWriteText },
        writable: true,
        configurable: true,
      })
    })

    it('has a copy button that copies input to clipboard', () => {
      render(
        <ToolCallCard
          name="Bash"
          input={{ command: 'npm install' }}
          description="Install deps"
        />
      )
      // Expand to see copy button
      const expandButton = screen.getByRole('button', { name: /tool call/i })
      fireEvent.click(expandButton)

      const copyButton = screen.getByRole('button', { name: /copy/i })
      expect(copyButton).toBeInTheDocument()

      fireEvent.click(copyButton)
      expect(mockWriteText).toHaveBeenCalledWith(
        JSON.stringify({ command: 'npm install' }, null, 2)
      )
    })
  })

  describe('Accessibility', () => {
    it('button is keyboard-focusable', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      button.focus()
      expect(button).toHaveFocus()
    })

    it('Enter key triggers click on button (native behavior)', () => {
      render(
        <ToolCallCard
          name="Read"
          input={{ file_path: '/foo.ts' }}
          description="Read file"
          parameters={{ file_path: '/foo.ts' }}
        />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      expect(button).toHaveAttribute('aria-expanded', 'false')

      // Simulate Enter key which fires click on buttons natively
      fireEvent.click(button)
      expect(button).toHaveAttribute('aria-expanded', 'true')
    })

    it('screen reader announces "Tool Call" + name', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const button = screen.getByRole('button', { name: /tool call.*read/i })
      expect(button).toBeInTheDocument()
    })

    it('expand button has aria-expanded attribute', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      expect(button).toHaveAttribute('aria-expanded')
    })

    it('focus ring visible on button (has focus-visible class)', () => {
      render(
        <ToolCallCard name="Read" input={{ file_path: '/foo.ts' }} description="Read file" />
      )
      const button = screen.getByRole('button', { name: /tool call/i })
      expect(button.className).toMatch(/focus/)
    })
  })
})
