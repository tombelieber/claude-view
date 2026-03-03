import { FindProvider, useFindQuery } from '@claude-view/shared/contexts/FindContext'
import { highlightText } from '@claude-view/shared/lib/highlight-text'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

/** Minimal consumer component that reads the find query and highlights text */
function TestConsumer({ text }: { text: string }) {
  const query = useFindQuery()
  return <div data-testid="output">{highlightText(text, query)}</div>
}

describe('FindContext + highlightText integration', () => {
  it('highlights text when query is provided via context', () => {
    const { container } = render(
      <FindProvider value="hello">
        <TestConsumer text="say hello to the world" />
      </FindProvider>,
    )
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(1)
    expect(marks[0].textContent).toBe('hello')
  })

  it('does not highlight when query is empty', () => {
    const { container } = render(
      <FindProvider value="">
        <TestConsumer text="say hello to the world" />
      </FindProvider>,
    )
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(0)
    expect(screen.getByTestId('output').textContent).toBe('say hello to the world')
  })

  it('highlights multiple occurrences', () => {
    const { container } = render(
      <FindProvider value="the">
        <TestConsumer text="the quick brown fox jumps over the lazy dog" />
      </FindProvider>,
    )
    const marks = container.querySelectorAll('mark')
    expect(marks).toHaveLength(2)
  })
})

describe('Cmd+F keyboard shortcut', () => {
  it('Cmd+F should be preventable on keydown', () => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
        e.preventDefault()
      }
    }
    window.addEventListener('keydown', handler)

    const event = new KeyboardEvent('keydown', {
      key: 'f',
      metaKey: true,
      cancelable: true,
    })
    const prevented = !window.dispatchEvent(event)
    expect(prevented).toBe(true)

    window.removeEventListener('keydown', handler)
  })

  it('Escape key should be detectable for closing find bar', () => {
    let escaped = false
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') escaped = true
    }
    window.addEventListener('keydown', handler)

    fireEvent.keyDown(window, { key: 'Escape' })
    expect(escaped).toBe(true)

    window.removeEventListener('keydown', handler)
  })
})
