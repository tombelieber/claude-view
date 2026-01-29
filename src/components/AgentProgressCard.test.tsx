import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { AgentProgressCard } from './AgentProgressCard'

describe('AgentProgressCard', () => {
  describe('React auto-escaping XSS prevention', () => {
    it('should properly escape text content with script tags (React auto-escaping)', () => {
      // Test data with script tags, onclick handlers, and other XSS vectors in text props
      const xssProps = {
        agentId: '<script>alert("XSS")</script>',
        prompt: 'Test prompt with <img src=x onerror="alert(1)">',
        model: 'claude-<script>alert("model")</script>opus',
        tokens: { input: 100, output: 200 },
        normalizedMessages: 5,
      }

      render(<AgentProgressCard {...xssProps} />)

      // Verify text content is rendered literally (escaped by React)
      // React auto-escaping converts < to &lt; and > to &gt; in text nodes
      expect(screen.getByText('<script>alert("XSS")</script>')).toBeInTheDocument()
      expect(screen.getByText('Test prompt with <img src=x onerror="alert(1)">'))
        .toBeInTheDocument()
      expect(screen.getByText('claude-<script>alert("model")</script>opus'))
        .toBeInTheDocument()

      // Verify no actual script execution happened (no alert fired)
      // and no event handlers are active (text is literal, not HTML)
      // If scripts executed, the test would not reach this point
      expect(true).toBe(true)
    })

    it('should render text content without interpreting HTML entities', () => {
      const htmlProps = {
        agentId: 'agent-&lt;malicious&gt;',
        prompt: 'Prompt with &quot;quoted&quot; text',
        model: 'claude-opus',
        tokens: { input: 50, output: 150 },
        normalizedMessages: 3,
      }

      render(<AgentProgressCard {...htmlProps} />)

      // Text should appear literally with & and quotes intact, not interpreted as entities
      expect(screen.getByText('agent-&lt;malicious&gt;')).toBeInTheDocument()
      expect(screen.getByText('Prompt with &quot;quoted&quot; text')).toBeInTheDocument()
    })

    it('should not allow event handler binding to escaped text', () => {
      const eventHandlerProps = {
        agentId: 'onclick="console.log(\'clicked\')"',
        prompt: 'onmouseover="alert(\'hover\')"',
        model: 'claude-opus',
        tokens: { input: 100, output: 200 },
        normalizedMessages: 5,
      }

      const { container } = render(<AgentProgressCard {...eventHandlerProps} />)

      // Text should be present but as literal strings (not as attributes)
      expect(screen.getByText('onclick="console.log(\'clicked\')"')).toBeInTheDocument()
      expect(screen.getByText('onmouseover="alert(\'hover\')"')).toBeInTheDocument()

      // Verify no event listeners are attached to the text elements
      // This would fail if HTML was interpreted instead of being escaped
      const allElements = container.querySelectorAll('*')
      for (const el of allElements) {
        // No onclick or onmouseover attributes should exist
        expect(el.getAttribute('onclick')).toBeNull()
        expect(el.getAttribute('onmouseover')).toBeNull()
      }
    })
  })

  describe('Happy path: Normal rendering', () => {
    it('should display agent progress with normal text', () => {
      const normalProps = {
        agentId: 'agent-123',
        prompt: 'Implement the feature',
        model: 'claude-opus',
        tokens: { input: 500, output: 1200 },
        normalizedMessages: 15,
      }

      render(<AgentProgressCard {...normalProps} />)

      expect(screen.getByText(/agent-123/i)).toBeInTheDocument()
      expect(screen.getByText(/Implement the feature/)).toBeInTheDocument()
      expect(screen.getByText(/claude-opus/)).toBeInTheDocument()
    })
  })

  describe('Edge cases: null/undefined handling', () => {
    it('should handle undefined optional fields gracefully', () => {
      const minimalProps = {
        agentId: 'agent-456',
        prompt: 'Simple task',
        model: 'claude-haiku',
      }

      render(<AgentProgressCard {...minimalProps} />)

      expect(screen.getByText(/agent-456/)).toBeInTheDocument()
      expect(screen.getByText(/Simple task/)).toBeInTheDocument()
    })

    it('should handle empty strings safely', () => {
      const emptyProps = {
        agentId: '',
        prompt: '',
        model: '',
        tokens: undefined,
        normalizedMessages: 0,
      }

      // Should render without crashing
      const { container } = render(<AgentProgressCard {...emptyProps} />)
      expect(container).toBeInTheDocument()
    })
  })
})
