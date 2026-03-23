import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { McpProgressCard } from './McpProgressCard'

describe('McpProgressCard', () => {
  describe('All fields inline', () => {
    it('should display serverName, toolName, and status', () => {
      render(<McpProgressCard serverName="filesystem" toolName="readFile" status="running" />)

      expect(screen.getByText('filesystem')).toBeInTheDocument()
      expect(screen.getByText('readFile')).toBeInTheDocument()
      expect(screen.getByText('running')).toBeInTheDocument()
    })
  })

  describe('Status styling', () => {
    it('should show pulse dot for running', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="running" />)

      const statusEl = screen.getByText('running')
      const dot = statusEl.querySelector('.animate-pulse')
      expect(dot).toBeInTheDocument()
    })

    it('should show green for completed', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="completed" />)

      expect(screen.getByText('completed').className).toContain('text-green')
    })

    it('should show red for error', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="error" />)

      expect(screen.getByText('error').className).toContain('text-red')
    })
  })

  describe('Visual styling', () => {
    it('should have blue left border', () => {
      const { container } = render(
        <McpProgressCard serverName="fs" toolName="read" status="running" />,
      )

      expect((container.firstElementChild as HTMLElement).className).toContain('flex')
    })
  })

  describe('ARIA', () => {
    it('should have ARIA label', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="running" />)

      expect(screen.getByLabelText('MCP tool call')).toBeInTheDocument()
    })
  })
})
