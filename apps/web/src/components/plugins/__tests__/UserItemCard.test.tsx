import { render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import { UserItemCard } from '../UserItemCard'

// Note: totalInvocations and sessionCount are bigint in the generated UserItemInfo type
const item = {
  name: 'prove-it',
  kind: 'skill',
  path: 'prove-it/SKILL.md',
  totalInvocations: BigInt(142),
  sessionCount: BigInt(38),
  lastUsedAt: BigInt(Math.floor(Date.now() / 1000) - 7200),
}

test('renders item name and kind badge', () => {
  render(<UserItemCard item={item} />)
  expect(screen.getByText('prove-it')).toBeInTheDocument()
  expect(screen.getByText('SKILL')).toBeInTheDocument()
})

test('renders usage stats when invocations > 0', () => {
  render(<UserItemCard item={item} />)
  expect(screen.getByText(/142/)).toBeInTheDocument()
})

test('renders path in mono font', () => {
  render(<UserItemCard item={item} />)
  expect(screen.getByText('prove-it/SKILL.md')).toBeInTheDocument()
})

test('renders "No usage in 30 days" in italic when invocations are 0', () => {
  render(
    <UserItemCard
      item={{ ...item, totalInvocations: BigInt(0), sessionCount: BigInt(0), lastUsedAt: null }}
    />,
  )
  expect(screen.getByText('No usage in 30 days')).toBeInTheDocument()
  const el = screen.getByText('No usage in 30 days')
  expect(el.className).toMatch(/italic/)
})

test('renders kebab menu button', () => {
  render(<UserItemCard item={item} />)
  expect(screen.getByText('···')).toBeInTheDocument()
})

test('uses smaller font for long names (>24 chars)', () => {
  const longItem = { ...item, name: 'full-codebase-docs-sync-scanner' }
  render(<UserItemCard item={longItem} />)
  const nameEl = screen.getByText('full-codebase-docs-sync-scanner')
  expect(nameEl.className).toMatch(/text-\[12px\]/)
})

test('card has faded opacity when unused', () => {
  const { container } = render(
    <UserItemCard
      item={{ ...item, totalInvocations: BigInt(0), sessionCount: BigInt(0), lastUsedAt: null }}
    />,
  )
  const button = container.querySelector('button')
  expect(button?.className).toMatch(/opacity-50/)
})
