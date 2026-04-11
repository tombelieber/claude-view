import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { CollapsibleJson } from '../CollapsibleJson'

describe('CollapsibleJson', () => {
  it('renders default label when no label prop given', () => {
    render(<CollapsibleJson data={{ x: 1 }} />)
    expect(screen.getByText('JSON')).toBeInTheDocument()
  })

  it('renders custom label', () => {
    render(<CollapsibleJson data={{ x: 1 }} label="Tool Input" />)
    expect(screen.getByText('Tool Input')).toBeInTheDocument()
  })

  it('is collapsed by default — JSON content not visible', () => {
    render(<CollapsibleJson data={{ foo: 'bar' }} />)
    expect(screen.queryByText(/"foo"/)).not.toBeInTheDocument()
  })

  it('expands and shows JSON content when clicked', () => {
    render(<CollapsibleJson data={{ foo: 'bar' }} />)
    fireEvent.click(screen.getByRole('button'))
    expect(screen.getByText(/"foo"/)).toBeInTheDocument()
  })

  it('collapses again when clicked a second time', () => {
    render(<CollapsibleJson data={{ foo: 'bar' }} />)
    const btn = screen.getByRole('button')
    fireEvent.click(btn)
    expect(screen.getByText(/"foo"/)).toBeInTheDocument()
    fireEvent.click(btn)
    expect(screen.queryByText(/"foo"/)).not.toBeInTheDocument()
  })

  it('is open by default when defaultOpen=true', () => {
    render(<CollapsibleJson data={{ hello: 'world' }} defaultOpen />)
    expect(screen.getByText(/"hello"/)).toBeInTheDocument()
  })

  it('renders non-object data (array)', () => {
    render(<CollapsibleJson data={[1, 2, 3]} defaultOpen />)
    expect(screen.getByText('1')).toBeInTheDocument()
  })
})
