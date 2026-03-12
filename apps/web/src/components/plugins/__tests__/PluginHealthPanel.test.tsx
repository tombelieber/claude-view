import { fireEvent, render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import { PluginHealthPanel } from '../PluginHealthPanel'

test('renders all three rows when counts > 0', () => {
  render(<PluginHealthPanel orphanCount={1} conflictCount={16} unusedCount={28} cliError={null} />)
  expect(screen.getByText(/1 orphaned/)).toBeInTheDocument()
  expect(screen.getByText(/16 conflicts/)).toBeInTheDocument()
  expect(screen.getByText(/28 unused/)).toBeInTheDocument()
})

test('renders nothing when all counts are zero and no cliError', () => {
  const { container } = render(
    <PluginHealthPanel orphanCount={0} conflictCount={0} unusedCount={0} cliError={null} />,
  )
  expect(container.firstChild).toBeNull()
})

test('shows cli error when present', () => {
  render(
    <PluginHealthPanel
      orphanCount={0}
      conflictCount={0}
      unusedCount={0}
      cliError="CLI not found"
    />,
  )
  expect(screen.getByText(/CLI not found/)).toBeInTheDocument()
})

test('collapses body when header clicked', () => {
  render(<PluginHealthPanel orphanCount={1} conflictCount={0} unusedCount={0} cliError={null} />)
  // Body visible initially (open=true by default)
  expect(screen.getByText(/1 orphaned/)).toBeInTheDocument()
  // Click the header toggle
  fireEvent.click(screen.getByText('Plugin health'))
  // Body should be hidden
  expect(screen.queryByText(/1 orphaned/)).not.toBeInTheDocument()
  // Click again to re-open
  fireEvent.click(screen.getByText('Plugin health'))
  expect(screen.getByText(/1 orphaned/)).toBeInTheDocument()
})

test('shows correct issue count', () => {
  render(<PluginHealthPanel orphanCount={1} conflictCount={16} unusedCount={0} cliError={null} />)
  expect(screen.getByText(/2 issues/)).toBeInTheDocument()
})

test('shows singular "issue" when only one category', () => {
  render(<PluginHealthPanel orphanCount={0} conflictCount={0} unusedCount={5} cliError={null} />)
  expect(screen.getByText(/1 issue(?!s)/)).toBeInTheDocument()
})
