import { render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import { SectionHeader } from '../SectionHeader'

test('renders title, count, and path', () => {
  render(<SectionHeader title="Skills" count={23} pathHint="~/.claude/skills/" />)
  expect(screen.getByText('Skills')).toBeInTheDocument()
  expect(screen.getByText('23')).toBeInTheDocument()
  expect(screen.getByText('~/.claude/skills/')).toBeInTheDocument()
})
