import { render, screen } from '@testing-library/react'
import { expect, test, vi } from 'vitest'
import { AvailablePluginCard } from '../AvailablePluginCard'

const baseAvailable = {
  pluginId: 'official:sentry',
  name: 'sentry',
  marketplaceName: 'claude-plugins-official',
  version: '1.0.0',
  description: 'Monitor Sentry errors',
  alreadyInstalled: false,
  installCount: BigInt(4200),
}

test('onInstall receives plugin.name, not plugin.pluginId', () => {
  const onInstall = vi.fn()
  render(<AvailablePluginCard plugin={baseAvailable} onInstall={onInstall} isPending={false} />)
  screen.getByText('GET').click()
  // MUST pass plugin.name ('sentry'), NOT plugin.pluginId ('official:sentry')
  expect(onInstall).toHaveBeenCalledWith('sentry', 'user')
})

test('shows INSTALLED badge when already installed', () => {
  render(
    <AvailablePluginCard
      plugin={{ ...baseAvailable, alreadyInstalled: true }}
      onInstall={() => {}}
      isPending={false}
    />,
  )
  expect(screen.getByText('INSTALLED')).toBeInTheDocument()
  expect(screen.queryByText('GET')).not.toBeInTheDocument()
})

test('shows install count formatted', () => {
  render(<AvailablePluginCard plugin={baseAvailable} onInstall={() => {}} isPending={false} />)
  expect(screen.getByText('4.2K installs')).toBeInTheDocument()
})
