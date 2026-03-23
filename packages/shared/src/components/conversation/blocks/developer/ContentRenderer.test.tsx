import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ContentRenderer } from './ContentRenderer'

describe('ContentRenderer', () => {
  it('renders JSON content as pretty-printed', () => {
    const json = JSON.stringify({ key: 'value', num: 42 })
    render(<ContentRenderer content={json} />)
    // Pretty-printed JSON should have the key on its own line
    expect(screen.getByText(/"key": "value"/)).toBeInTheDocument()
  })

  it('renders diff content with colored lines', () => {
    const diff = `diff --git a/file.ts b/file.ts
--- a/file.ts
+++ b/file.ts
@@ -1,3 +1,3 @@
 unchanged
-removed line
+added line`
    const { container } = render(<ContentRenderer content={diff} />)
    // + lines should have green color class
    const addedLine = container.querySelector('.text-green-400')
    expect(addedLine).toBeInTheDocument()
    // - lines should have red color class
    const removedLine = container.querySelector('.text-red-400')
    expect(removedLine).toBeInTheDocument()
  })

  it('renders plain text', () => {
    render(<ContentRenderer content="Hello plain text" />)
    expect(screen.getByText('Hello plain text')).toBeInTheDocument()
  })

  it('truncates content over 2000 chars', () => {
    const longContent = 'x'.repeat(2500)
    render(<ContentRenderer content={longContent} />)
    const pre = screen.getByText(/\.\.\.truncated/)
    expect(pre).toBeInTheDocument()
  })

  it('returns null for empty content', () => {
    const { container } = render(<ContentRenderer content="" />)
    expect(container.firstChild).toBeNull()
  })
})
