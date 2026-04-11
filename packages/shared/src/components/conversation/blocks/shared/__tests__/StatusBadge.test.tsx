import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { StatusBadge } from '../StatusBadge'

describe('StatusBadge', () => {
  it('renders the label text', () => {
    render(<StatusBadge label="active" />)
    expect(screen.getByText('active')).toBeInTheDocument()
  })

  it('applies default gray color classes when no color given', () => {
    const { container } = render(<StatusBadge label="unknown" />)
    const span = container.querySelector('span')
    expect(span?.className).toContain('text-gray-600')
    expect(span?.className).toContain('dark:text-gray-400')
  })

  it.each([
    'gray',
    'green',
    'red',
    'amber',
    'blue',
    'cyan',
    'teal',
    'orange',
    'indigo',
    'violet',
    'purple',
  ] as const)('renders color variant: %s', (color) => {
    const { container } = render(<StatusBadge label="test" color={color} />)
    const span = container.querySelector('span')
    expect(span?.className).toContain(`text-${color}-600`)
    expect(span?.className).toContain(`dark:text-${color}-400`)
    expect(span?.className).toContain(`bg-${color}-500/10`)
  })

  it('applies base typography classes', () => {
    const { container } = render(<StatusBadge label="x" />)
    const span = container.querySelector('span')
    expect(span?.className).toContain('text-xs')
    expect(span?.className).toContain('font-mono')
    expect(span?.className).toContain('px-1.5')
    expect(span?.className).toContain('py-0.5')
    expect(span?.className).toContain('rounded')
  })
})
