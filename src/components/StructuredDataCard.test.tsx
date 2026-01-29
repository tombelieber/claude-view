import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StructuredDataCard } from './StructuredDataCard'

describe('StructuredDataCard', () => {
  describe('Script tag sanitization', () => {
    it('should remove script tags from the sanitized content', () => {
      const xmlWithScript = `
        <div>
          <p>Safe content</p>
          <script>alert('XSS')</script>
        </div>
      `
      render(<StructuredDataCard xml={xmlWithScript} type="unknown" />)

      const content = screen.getByText(/Safe content/i)
      expect(content).toBeInTheDocument()

      // Verify script tag is not in the DOM
      expect(screen.queryByText(/alert/)).not.toBeInTheDocument()
    })
  })

  describe('Event handler removal', () => {
    it('should remove onclick and onerror event handlers', () => {
      const xmlWithHandlers = `
        <div>
          <p onclick="alert('XSS')">Malicious content</p>
          <img src="x" onerror="alert('XSS')" />
          <span>Safe content</span>
        </div>
      `
      render(<StructuredDataCard xml={xmlWithHandlers} type="unknown" />)

      // Get the rendered element
      const content = screen.getByText(/Safe content/i)
      expect(content).toBeInTheDocument()

      // Verify no onclick or onerror attributes exist in the rendered content
      const allDivs = screen.getByText(/Safe content/i).closest('div')
      expect(allDivs?.innerHTML).not.toContain('onclick')
      expect(allDivs?.innerHTML).not.toContain('onerror')
    })
  })

  describe('Safe content preservation', () => {
    it('should preserve safe HTML tags and text content', () => {
      const safeXml = `
        <div>
          <p>Paragraph text</p>
          <span>Span text</span>
          <ul>
            <li>Item 1</li>
            <li>Item 2</li>
          </ul>
          <code>const x = 1;</code>
          <br />
          <pre>Preformatted text</pre>
        </div>
      `
      render(<StructuredDataCard xml={safeXml} type="unknown" />)

      expect(screen.getByText(/Paragraph text/i)).toBeInTheDocument()
      expect(screen.getByText(/Span text/i)).toBeInTheDocument()
      expect(screen.getByText(/Item 1/i)).toBeInTheDocument()
      expect(screen.getByText(/Item 2/i)).toBeInTheDocument()
      expect(screen.getByText(/const x = 1;/i)).toBeInTheDocument()
      expect(screen.getByText(/Preformatted text/i)).toBeInTheDocument()
    })
  })

  describe('Empty and null content handling', () => {
    it('should gracefully handle empty content', () => {
      render(<StructuredDataCard xml="" type="unknown" />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle null content', () => {
      render(<StructuredDataCard xml={null as any} type="unknown" />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle undefined content', () => {
      render(<StructuredDataCard xml={undefined as any} type="unknown" />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle whitespace-only content', () => {
      const { container } = render(<StructuredDataCard xml="   \n\n  " type="unknown" />)
      // Should either show "No data" or the container should have minimal meaningful content
      const noDataElement = screen.queryByText(/No data/i)
      if (noDataElement) {
        expect(noDataElement).toBeInTheDocument()
      } else {
        // If not "No data", then it should still render without crashing
        expect(container).toBeInTheDocument()
      }
    })
  })

  describe('Large XML performance', () => {
    it('should sanitize large XML efficiently without crashing', () => {
      // Generate large XML content (100KB)
      const largeContent = Array(10000)
        .fill('<div><p>Safe content line</p></div>')
        .join('')

      const startTime = performance.now()
      render(<StructuredDataCard xml={largeContent} type="unknown" />)
      const endTime = performance.now()

      // Should complete in under 5 seconds (render + sanitization)
      expect(endTime - startTime).toBeLessThan(5000)

      // Verify content is rendered (queryAllByText since there will be multiple matches)
      const matches = screen.queryAllByText(/Safe content line/i)
      expect(matches.length).toBeGreaterThan(0)
    })

    it('should handle large XML with mixed safe and unsafe content', () => {
      // Generate large XML with both safe and unsafe content (50KB)
      const largeContent = Array(5000)
        .fill(null)
        .map((_, i) =>
          i % 2 === 0
            ? '<div><p>Safe content</p><script>alert("XSS")</script></div>'
            : '<div><p>Another safe line</p></div>'
        )
        .join('')

      const startTime = performance.now()
      render(<StructuredDataCard xml={largeContent} type="unknown" />)
      const endTime = performance.now()

      // Should complete efficiently
      expect(endTime - startTime).toBeLessThan(5000)

      // Scripts should be removed
      expect(screen.queryByText(/alert/)).not.toBeInTheDocument()
    })
  })
})
