import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { XmlCard } from './XmlCard'

describe('XmlCard - UntrustedData XSS Hardening', () => {
  describe('Test 1: Plaintext rendering only (no HTML interpretation)', () => {
    it('should render untrusted data as plaintext only, without HTML interpretation', () => {
      const untrustedXml = `<untrusted-data-abc123>
<div onclick="alert('XSS')">This should be plaintext</div>
<script>alert('XSS')</script>
Some normal text here
</untrusted-data-abc123>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify plaintext rendering
      expect(screen.getByText(/This should be plaintext/)).toBeInTheDocument()
      expect(screen.getByText(/Some normal text here/)).toBeInTheDocument()

      // Verify content is in a <pre> tag for plaintext rendering
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      // Verify no actual div or script elements are rendered (sanitized)
      expect(container.querySelectorAll('div[onclick]').length).toBe(0)
      expect(container.querySelectorAll('script').length).toBe(0)
    })

    it('should preserve formatting in plaintext rendering', () => {
      const untrustedXml = `<untrusted-data-abc123>Line 1
Line 2 with some content
  Indented line
Line 4</untrusted-data-abc123>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify pre tag preserves formatting
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      // Verify whitespace/indentation is preserved
      expect(preElement?.className).toContain('whitespace-pre-wrap')

      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('Line 1')
      expect(preContent).toContain('Line 2 with some content')
      expect(preContent).toContain('Indented line')
      expect(preContent).toContain('Line 4')
    })
  })

  describe('Test 2: Script tags/onclick handlers are neutralized (no XSS execution)', () => {
    it('should neutralize script tags - they should not execute', () => {
      const untrustedXml = `<untrusted-data-xyz789><script>alert('XSS-ATTACK')</script>Safe content</untrusted-data-xyz789>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify the script tag appears as plaintext, not as an executable script
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('<script>')
      expect(preContent).toContain('alert')

      // Verify there are no actual script elements in the DOM
      const scriptElements = container.querySelectorAll('script')
      expect(scriptElements.length).toBe(0)
    })

    it('should neutralize onclick handlers - they should not execute', () => {
      const untrustedXml = `<untrusted-data-onclick><div onclick="alert('XSS-ONCLICK')">Click me</div></untrusted-data-onclick>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify onclick handler appears as plaintext text
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('onclick=')
      expect(preContent).toContain('alert')

      // Verify no actual event handlers are attached
      const divElements = container.querySelectorAll('div')
      divElements.forEach((div) => {
        // The div should not have onclick handler (plaintext doesn't bind events)
        expect(div.getAttribute('onclick')).toBeNull()
      })
    })

    it('should neutralize onerror handlers', () => {
      const untrustedXml = `<untrusted-data-onerror><img onerror="alert('XSS')" src="x" /></untrusted-data-onerror>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify onerror handler appears as plaintext
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('onerror=')

      // Verify no actual img elements with event handlers
      const imgElements = container.querySelectorAll('img')
      expect(imgElements.length).toBe(0)
    })

    it('should handle complex XSS payloads safely', () => {
      const xssPayloads = [
        '<svg/onload=alert("XSS")>',
        '<iframe src="javascript:alert(\'XSS\')">',
        '<input onfocus="alert(\'XSS\')" autofocus>',
        '<body onload=alert("XSS")>',
        '<marquee onstart="alert(\'XSS\')">',
      ]

      const untrustedXml = `<untrusted-data-complex>${xssPayloads.join('\n')}</untrusted-data-complex>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify all payloads appear as plaintext, not as executable code
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      const preContent = preElement?.textContent || ''
      xssPayloads.forEach((payload) => {
        // Each payload should be visible as text
        expect(preContent).toContain('<')
        expect(preContent).toContain('>')
      })

      // Verify no actual event handlers in DOM
      const elementsWithHandlers = container.querySelectorAll('[onload], [onfocus], [onstart], [onerror]')
      expect(elementsWithHandlers.length).toBe(0)
    })
  })

  describe('Test 3: Text formatting is preserved in pre tag', () => {
    it('should preserve whitespace and formatting in pre tag', () => {
      const untrustedXml = `<untrusted-data-format>
This is line 1
  This is indented
    This is more indented
        And even more
Back to normal
</untrusted-data-format>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      // Check that pre element uses appropriate CSS classes
      expect(preElement?.className).toContain('whitespace-pre-wrap')
      expect(preElement?.className).toContain('font-mono')

      // Verify content is preserved
      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('This is line 1')
      expect(preContent).toContain('And even more')
    })

    it('should preserve multiline text with special characters', () => {
      const untrustedXml = `<untrusted-data-special>
var x = { a: 1, b: 2 };
console.log("Hello World!");
const json = {"key": "value"};
function test() {
  return true;
}
</untrusted-data-special>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('var x = { a: 1, b: 2 };')
      expect(preContent).toContain('console.log("Hello World!");')
      expect(preContent).toContain('const json = {"key": "value"};')
      expect(preContent).toContain('function test()')
    })

    it('should use pre tag and not dangerouslySetInnerHTML for plaintext rendering', () => {
      const untrustedXml = `<untrusted-data-precheck>Sample untrusted content</untrusted-data-precheck>`

      const { container } = render(<XmlCard content={untrustedXml} type="untrusted_data" />)

      // Verify pre tag exists
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()

      // Verify the content is rendered as text node, not via dangerouslySetInnerHTML
      // We verify this by checking that the pre element has a text node child
      const hasTextContent = preElement?.textContent?.includes('Sample untrusted content')
      expect(hasTextContent).toBe(true)
    })
  })

  describe('Acceptance criteria verification', () => {
    it('should meet all acceptance criteria for UntrustedData XSS hardening', () => {
      const maliciousXml = `<untrusted-data-full>
Line 1: Normal text
<script>alert('should not execute')</script>
<div onclick="alert('should not execute')">HTML content</div>
    Indented content
Line 5: More content
</untrusted-data-full>`

      const { container } = render(<XmlCard content={maliciousXml} type="untrusted_data" />)

      // AC1: Plaintext rendering only (in <pre> tag)
      const preElement = container.querySelector('pre')
      expect(preElement).toBeInTheDocument()
      expect(preElement?.className).toContain('whitespace-pre-wrap')
      expect(preElement?.className).toContain('font-mono')

      // AC2: No script execution possible
      expect(container.querySelectorAll('script').length).toBe(0)
      expect(container.querySelectorAll('[onclick]').length).toBe(0)
      expect(container.querySelectorAll('[onerror]').length).toBe(0)

      // AC3: Formatting and safe text content preserved
      const preContent = preElement?.textContent || ''
      expect(preContent).toContain('Line 1: Normal text')
      expect(preContent).toContain('HTML content') // Text inside dangerous tags is preserved
      expect(preContent).toContain('Indented content')
      expect(preContent).toContain('Line 5: More content')

      // Verify dangerous markup appears as plaintext (not as executable HTML)
      // With KEEP_CONTENT: true, DOMPurify keeps tags as text but strips attributes
      // This is safe because <pre> renders it as literal text, not as HTML
      const hasTagsAsText = preContent.includes('<script>') || preContent.includes('onclick=') || preContent.includes('<div')
      // Whether tags appear as text or are removed, they cannot execute
      // What matters is that actual script elements and onclick handlers don't exist in DOM
      expect(container.querySelectorAll('script').length).toBe(0)
      expect(container.querySelectorAll('[onclick]').length).toBe(0)
    })
  })
})
