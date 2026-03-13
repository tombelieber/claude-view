import { render, screen } from '@testing-library/react'
import { expect, test } from 'vitest'
import { PluginToolbar } from '../PluginToolbar'

test('active tab has white pill class', () => {
  render(
    <PluginToolbar
      search=""
      onSearchChange={() => {}}
      scope={undefined}
      onScopeChange={() => {}}
      source={undefined}
      onSourceChange={() => {}}
      kind="skill"
      onKindChange={() => {}}
      marketplaces={[]}
      totalCount={0}
      kindCounts={{ plugin: 0, skill: 0, command: 0, agent: 0, mcp_tool: 0 }}
    />,
  )
  // The "Skills" tab should have active styling (bg-white)
  const skillTab = screen.getByText(/Skills/i).closest('button')
  expect(skillTab?.className).toMatch(/bg-white/)
})
