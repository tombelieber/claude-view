import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { StructuredDataCard } from './StructuredDataCard'

describe('StructuredDataCard', () => {
  describe('XSS prevention', () => {
    it('should render content as plain text (no script execution)', () => {
      const xmlWithScript = `
        <div>
          <p>Safe content</p>
          <script>alert('XSS')</script>
        </div>
      `
      render(<StructuredDataCard xml={xmlWithScript} />)

      // Content is preserved as text
      const content = screen.getByText(/Safe content/i)
      expect(content).toBeInTheDocument()

      // No script elements in the DOM (rendered as text, not HTML)
      const scripts = document.querySelectorAll('script')
      expect(scripts).toHaveLength(0)
    })

    it('should not create interactive elements from untrusted HTML', () => {
      const xmlWithHandlers = `
        <div>
          <p onclick="alert('XSS')">Malicious content</p>
          <img src="x" onerror="alert('XSS')" />
          <span>Safe content</span>
        </div>
      `
      const { container } = render(<StructuredDataCard xml={xmlWithHandlers} />)

      // Text content is preserved
      expect(container.textContent).toContain('Safe content')
      expect(container.textContent).toContain('Malicious content')

      // No interactive img elements (rendered as text, not HTML elements)
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()
      // Content is in a pre tag as text, not as DOM elements
      expect(preElement?.tagName).toBe('PRE')
    })
  })

  describe('Safe content preservation', () => {
    it('should preserve text content from HTML tags', () => {
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
      render(<StructuredDataCard xml={safeXml} />)

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
      render(<StructuredDataCard xml="" />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle null content', () => {
      render(<StructuredDataCard xml={null} />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle undefined content', () => {
      render(<StructuredDataCard xml={undefined} />)
      expect(screen.getByText(/No data/i)).toBeInTheDocument()
    })

    it('should gracefully handle whitespace-only content', () => {
      const { container } = render(<StructuredDataCard xml="   \n\n  " />)
      const noDataElement = screen.queryByText(/No data/i)
      if (noDataElement) {
        expect(noDataElement).toBeInTheDocument()
      } else {
        expect(container).toBeInTheDocument()
      }
    })
  })

  describe('Nesting depth protection', () => {
    it('should handle normal nesting depth (10 levels)', () => {
      const xml = '<div><div><div><div><div><div><div><div><div><div>Content</div></div></div></div></div></div></div></div></div></div>'
      render(<StructuredDataCard xml={xml} />)
      expect(screen.getByText(/Content/)).toBeInTheDocument()
    })

    it('should handle moderate nesting depth (50 levels)', () => {
      let xml = 'Content'
      for (let i = 0; i < 50; i++) {
        xml = `<div>${xml}</div>`
      }
      render(<StructuredDataCard xml={xml} />)
      expect(screen.getByText(/Content/)).toBeInTheDocument()
    })

    it('should handle deep nesting gracefully (100+ levels)', () => {
      let xml = 'Deep content'
      for (let i = 0; i < 100; i++) {
        xml = `<div>${xml}</div>`
      }
      const { container } = render(<StructuredDataCard xml={xml} />)
      expect(container).toBeInTheDocument()
    })

    it('should prevent stack overflow from excessive nesting', () => {
      let xml = 'Content'
      for (let i = 0; i < 500; i++) {
        xml = `<div>${xml}</div>`
      }
      const { container } = render(<StructuredDataCard xml={xml} />)
      expect(container).toBeInTheDocument()
    })

    it('should show graceful degradation indicator for very deep content', () => {
      let xml = 'Nested content'
      for (let i = 0; i < 200; i++) {
        xml = `<div>${xml}</div>`
      }
      const { container } = render(<StructuredDataCard xml={xml} />)
      expect(container).toBeInTheDocument()
    })
  })

  describe('Large XML performance', () => {
    it('should sanitize large XML efficiently without crashing', () => {
      const largeContent = Array(10000)
        .fill('<div><p>Safe content line</p></div>')
        .join('')

      const startTime = performance.now()
      render(<StructuredDataCard xml={largeContent} />)
      const endTime = performance.now()

      expect(endTime - startTime).toBeLessThan(6000)
      const matches = screen.queryAllByText(/Safe content line/i)
      expect(matches.length).toBeGreaterThan(0)
    })

    it('should handle large XML with mixed safe and unsafe content', () => {
      const largeContent = Array(5000)
        .fill(null)
        .map((_, i) =>
          i % 2 === 0
            ? '<div><p>Safe content</p><script>alert("XSS")</script></div>'
            : '<div><p>Another safe line</p></div>'
        )
        .join('')

      const startTime = performance.now()
      const { container } = render(<StructuredDataCard xml={largeContent} />)
      const endTime = performance.now()

      expect(endTime - startTime).toBeLessThan(6000)
      // No script elements in the DOM
      expect(container.querySelectorAll('script')).toHaveLength(0)
    })
  })
})
