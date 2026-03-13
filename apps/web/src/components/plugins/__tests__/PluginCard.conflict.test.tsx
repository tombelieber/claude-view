import { fireEvent, render, screen } from '@testing-library/react'
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
  sourceExists: true,
  description: null,
  installCount: null,
}

test('shows clear conflict message not "Also from"', () => {
  render(<PluginCard plugin={basePlugin} onAction={() => {}} isPending={false} />)
  expect(screen.queryByText(/Also from/)).not.toBeInTheDocument()
  expect(screen.getByText(/Conflict:/)).toBeInTheDocument()
  expect(screen.getByText(/This version runs/)).toBeInTheDocument()
})

// Local plugin: errors reported by CLI but installPath exists on disk (sourceExists=true)
const localErrorPlugin = {
  ...basePlugin,
  duplicateMarketplaces: [],
  errors: ['Plugin wtf not found in marketplace local'],
  sourceExists: true,
}

test('plugin with errors but intact source shows "CLI verification issue", no Reinstall', () => {
  render(<PluginCard plugin={localErrorPlugin} onAction={() => {}} isPending={false} />)
  expect(screen.queryByText(/Orphaned install/)).not.toBeInTheDocument()
  expect(screen.getByText(/CLI verification issue/)).toBeInTheDocument()
  expect(screen.getByText(/files are intact/)).toBeInTheDocument()
  expect(screen.queryByText('Reinstall')).not.toBeInTheDocument()
  expect(screen.getByText('Remove')).toBeInTheDocument()
})

test('intact-source Remove button calls uninstall', () => {
  const onAction = vi.fn()
  render(<PluginCard plugin={localErrorPlugin} onAction={onAction} isPending={false} />)
  fireEvent.click(screen.getByText('Remove'))
  expect(onAction).toHaveBeenCalledWith('uninstall', 'test', 'user', null)
})

// Truly orphaned: installPath deleted, sourceExists=false
const trueOrphanPlugin = {
  ...basePlugin,
  marketplace: 'claude-plugins-official',
  duplicateMarketplaces: [],
  errors: ['Source path missing'],
  sourceExists: false,
}

test('truly orphaned plugin (sourceExists=false) shows "Orphaned install" with Reinstall', () => {
  render(<PluginCard plugin={trueOrphanPlugin} onAction={() => {}} isPending={false} />)
  expect(screen.getByText('Orphaned install')).toBeInTheDocument()
  expect(screen.getByText(/Source directory missing/)).toBeInTheDocument()
  expect(screen.getByText('Reinstall')).toBeInTheDocument()
  expect(screen.getByText('Remove')).toBeInTheDocument()
})

test('orphan Reinstall button calls install action', () => {
  const onAction = vi.fn()
  render(<PluginCard plugin={trueOrphanPlugin} onAction={onAction} isPending={false} />)
  fireEvent.click(screen.getByText('Reinstall'))
  expect(onAction).toHaveBeenCalledWith('install', 'test', 'user')
})
