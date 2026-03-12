import { render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'
import { PluginCard } from '../PluginCard'

const basePlugin = {
  id: 'test@local',
  name: 'test',
  marketplace: 'local',
  scope: 'user',
  enabled: true,
  version: '1.0.0',
  gitSha: null,
  installedAt: '2026-01-01T00:00:00Z',
  lastUpdated: null,
  projectPath: null,
  items: [],
  skillCount: 0,
  commandCount: 0,
  agentCount: 0,
  mcpCount: 0,
  totalInvocations: BigInt(0),
  sessionCount: BigInt(0),
  lastUsedAt: null,
  duplicateMarketplaces: ['claude-plugins-official'],
  updatable: false,
  errors: [],
  description: null,
  installCount: null,
}

test('shows clear conflict message not "Also from"', () => {
  render(<PluginCard plugin={basePlugin} onAction={() => {}} isPending={false} />)
  expect(screen.queryByText(/Also from/)).not.toBeInTheDocument()
  expect(screen.getByText(/Conflict:/)).toBeInTheDocument()
  expect(screen.getByText(/This version runs/)).toBeInTheDocument()
})

const orphanPlugin = {
  ...basePlugin,
  duplicateMarketplaces: [],
  errors: ['Source path missing'],
}

test('shows orphan block with Reinstall and Remove buttons', () => {
  render(<PluginCard plugin={orphanPlugin} onAction={() => {}} isPending={false} />)
  expect(screen.getByText('Orphaned install')).toBeInTheDocument()
  expect(screen.getByText("Source path missing. Can't update or verify.")).toBeInTheDocument()
  expect(screen.getByText('Reinstall')).toBeInTheDocument()
  expect(screen.getByText('Remove')).toBeInTheDocument()
})

test('orphan Reinstall button calls onAction with reinstall', () => {
  const onAction = vi.fn()
  render(<PluginCard plugin={orphanPlugin} onAction={onAction} isPending={false} />)
  screen.getByText('Reinstall').click()
  expect(onAction).toHaveBeenCalledWith('reinstall', 'test', 'user')
})
