import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { McpProgressCard } from './McpProgressCard'

describe('McpProgressCard', () => {
  describe('Header rendering', () => {
    it('should display serverName.toolName', () => {
      render(<McpProgressCard serverName="filesystem" toolName="readFile" status="running" />)

      expect(screen.getByText(/filesystem\.readFile/)).toBeInTheDocument()
    })
  })

  describe('Status badge', () => {
    it('should show running status with pulse indicator', () => {
      const { container } = render(
        <McpProgressCard serverName="fs" toolName="read" status="running" />,
      )

      expect(screen.getByText('running')).toBeInTheDocument()
      // Pulse dot should exist
      const pulseDot = container.querySelector('.animate-pulse')
      expect(pulseDot).toBeInTheDocument()
    })

    it('should show completed status with green styling', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="completed" />)

      const badge = screen.getByText('completed')
      expect(badge.className).toContain('text-green')
    })

    it('should show error status with red styling', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="error" />)

      const badge = screen.getByText('error')
      expect(badge.className).toContain('text-red')
    })

    it('should handle unknown status with gray fallback', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="pending" />)

      const badge = screen.getByText('pending')
      expect(badge.className).toContain('text-gray')
    })

    it('should not show pulse for non-running status', () => {
      const { container } = render(
        <McpProgressCard serverName="fs" toolName="read" status="completed" />,
      )

      const pulseDot = container.querySelector('.animate-pulse')
      expect(pulseDot).not.toBeInTheDocument()
    })
  })

  describe('Visual styling', () => {
    it('should have blue left border', () => {
      const { container } = render(
        <McpProgressCard serverName="fs" toolName="read" status="running" />,
      )

      const card = container.firstElementChild as HTMLElement
      expect(card.className).toContain('border-l-blue')
    })
  })

  describe('ARIA', () => {
    it('should have ARIA label', () => {
      render(<McpProgressCard serverName="fs" toolName="read" status="running" />)

      expect(screen.getByLabelText('MCP tool call')).toBeInTheDocument()
    })
  })
})
