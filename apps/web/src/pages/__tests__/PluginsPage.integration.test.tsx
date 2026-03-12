import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeAll, expect, test, vi } from 'vitest'

// Mock the hooks — testing PAGE wiring, not the hooks themselves
vi.mock('../../hooks/use-plugins', () => ({
  usePlugins: () => ({
    data: {
      installed: [
        {
          id: 'superpowers@user',
          name: 'superpowers',
          marketplace: 'superpowers-marketplace',
          scope: 'user',
          enabled: true,
          version: '5.0.2',
          gitSha: null,
          installedAt: '2026-01-01T00:00:00Z',
          lastUpdated: null,
          projectPath: null,
          items: [],
          skillCount: 15,
          commandCount: 2,
          agentCount: 3,
          mcpCount: 0,
          totalInvocations: BigInt(1240),
          sessionCount: BigInt(38),
          lastUsedAt: BigInt(1741800000),
          duplicateMarketplaces: ['claude-plugins-official'],
          updatable: false,
          errors: [],
          description: 'Superpowers plugin',
          installCount: null,
        },
      ],
      available: [
        {
          pluginId: 'official:sentry',
          name: 'sentry',
          marketplaceName: 'claude-plugins-official',
          version: '1.0.0',
          description: 'Monitor Sentry errors',
          alreadyInstalled: false,
          installCount: BigInt(4200),
        },
      ],
      totalInstalled: 1,
      totalAvailable: 1,
      duplicateCount: 1,
      unusedCount: 0,
      updatableCount: 0,
      marketplaces: [],
      cliError: null,
      orphanCount: 0,
      userSkills: [
        {
          name: 'prove-it',
          kind: 'skill',
          path: 'prove-it/SKILL.md',
          totalInvocations: BigInt(142),
          sessionCount: BigInt(38),
          lastUsedAt: BigInt(1741800000),
        },
      ],
      userCommands: [
        {
          name: 'wtf',
          kind: 'command',
          path: 'commands/wtf.md',
          totalInvocations: BigInt(57),
          sessionCount: BigInt(19),
          lastUsedAt: BigInt(1741700000),
        },
      ],
      userAgents: [],
    },
    isLoading: false,
    error: null,
  }),
}))

vi.mock('../../hooks/use-plugin-mutations', () => ({
  usePluginMutations: () => ({
    execute: vi.fn(),
    isPending: false,
    pendingName: null,
  }),
}))

function renderPage(PluginsPage: React.ComponentType) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <MemoryRouter>
        <PluginsPage />
      </MemoryRouter>
    </QueryClientProvider>,
  )
}

let PluginsPage: React.ComponentType
beforeAll(async () => {
  const mod = await import('../PluginsPage')
  PluginsPage =
    (mod as { default?: React.ComponentType; PluginsPage?: React.ComponentType }).default ??
    (mod as { PluginsPage: React.ComponentType }).PluginsPage
})

test('renders page header with title and item count', () => {
  renderPage(PluginsPage)
  // "Plugins" appears in both the page heading and the toolbar tab — use getAllByText
  expect(screen.getAllByText('Plugins').length).toBeGreaterThan(0)
  // 1 installed + 1 available + 1 userSkill + 1 userCommand = 4
  expect(screen.getByText(/4 items/)).toBeInTheDocument()
})

test('renders Skills section with user skill', () => {
  renderPage(PluginsPage)
  // "Skills" appears in the toolbar tab AND as a section header
  expect(screen.getAllByText('Skills').length).toBeGreaterThan(0)
  expect(screen.getByText('prove-it')).toBeInTheDocument()
})

test('renders Commands section with user command', () => {
  renderPage(PluginsPage)
  // "Commands" appears in the toolbar tab AND as a section header
  expect(screen.getAllByText('Commands').length).toBeGreaterThan(0)
  expect(screen.getByText('wtf')).toBeInTheDocument()
})

test('renders Installed Plugins section', () => {
  renderPage(PluginsPage)
  expect(screen.getByText('Installed Plugins')).toBeInTheDocument()
  expect(screen.getByText('superpowers')).toBeInTheDocument()
})

test('renders Available section', () => {
  renderPage(PluginsPage)
  expect(screen.getByText('Available in Marketplaces')).toBeInTheDocument()
  expect(screen.getByText('sentry')).toBeInTheDocument()
})

test('renders health panel with conflict', () => {
  renderPage(PluginsPage)
  expect(screen.getByText('Plugin health')).toBeInTheDocument()
  expect(screen.getByText(/1 conflict/)).toBeInTheDocument()
})
