import { render, screen } from '@testing-library/react'
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

test('shows all items when count is small', () => {
  render(<UserItemSection title="Skills" items={makeItems(2)} pathPrefix="~/.claude/skills/" />)
  expect(screen.getByText('skill-0')).toBeInTheDocument()
  expect(screen.getByText('skill-1')).toBeInTheDocument()
})

test('shows all items without truncation', () => {
  render(<UserItemSection title="Skills" items={makeItems(6)} pathPrefix="~/.claude/skills/" />)
  for (let i = 0; i < 6; i++) {
    expect(screen.getByText(`skill-${i}`)).toBeInTheDocument()
  }
  expect(screen.queryByText(/more/)).not.toBeInTheDocument()
})

test('renders section header with title, count, and path', () => {
  render(<UserItemSection title="Agents" items={makeItems(1)} pathPrefix="~/.claude/agents/" />)
  expect(screen.getByText('Agents')).toBeInTheDocument()
  expect(screen.getByText('1')).toBeInTheDocument()
  expect(screen.getByText('~/.claude/agents/')).toBeInTheDocument()
})
