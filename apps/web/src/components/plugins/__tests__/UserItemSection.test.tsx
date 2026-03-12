import { fireEvent, render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import { UserItemSection } from '../UserItemSection'

const makeItems = (count: number) =>
  Array.from({ length: count }, (_, i) => ({
    name: `skill-${i}`,
    kind: 'skill',
    path: `skill-${i}/SKILL.md`,
    totalInvocations: BigInt(10),
    sessionCount: BigInt(3),
    lastUsedAt: BigInt(Math.floor(Date.now() / 1000)),
  }))

test('shows all items when <= 3', () => {
  render(<UserItemSection title="Skills" items={makeItems(2)} pathPrefix="~/.claude/skills/" />)
  expect(screen.getByText('skill-0')).toBeInTheDocument()
  expect(screen.getByText('skill-1')).toBeInTheDocument()
  expect(screen.queryByText(/more/)).not.toBeInTheDocument()
})

test('shows only 3 items + "show more" when > 3', () => {
  render(<UserItemSection title="Skills" items={makeItems(6)} pathPrefix="~/.claude/skills/" />)
  expect(screen.getByText('skill-0')).toBeInTheDocument()
  expect(screen.getByText('skill-2')).toBeInTheDocument()
  expect(screen.queryByText('skill-3')).not.toBeInTheDocument()
  expect(screen.getByText('+ 3 more skills')).toBeInTheDocument()
})

test('expands to show all items when "show more" clicked', () => {
  render(<UserItemSection title="Skills" items={makeItems(6)} pathPrefix="~/.claude/skills/" />)
  fireEvent.click(screen.getByText('+ 3 more skills'))
  expect(screen.getByText('skill-5')).toBeInTheDocument()
  expect(screen.queryByText(/more/)).not.toBeInTheDocument()
})

test('renders section header with title, count, and path', () => {
  render(<UserItemSection title="Agents" items={makeItems(1)} pathPrefix="~/.claude/agents/" />)
  expect(screen.getByText('Agents')).toBeInTheDocument()
  expect(screen.getByText('1')).toBeInTheDocument()
  expect(screen.getByText('~/.claude/agents/')).toBeInTheDocument()
})
