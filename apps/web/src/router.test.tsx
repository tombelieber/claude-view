import { render, screen } from '@testing-library/react'
import { RouterProvider, createMemoryRouter } from 'react-router-dom'
import { describe, expect, it, vi } from 'vitest'

vi.mock('./App', async () => {
  const { Outlet } = await import('react-router-dom')
  return {
    default: () => <Outlet />,
  }
})

vi.mock('./components/ConversationView', () => ({
  ConversationView: () => <div>Conversation View</div>,
}))

vi.mock('./components/HistoryView', () => ({
  HistoryView: () => <div>History View</div>,
}))

vi.mock('./components/SearchResults', () => ({
  SearchResults: () => <div>Search Results</div>,
}))

vi.mock('./components/SettingsPage', () => ({
  SettingsPage: () => <div>Settings Page</div>,
}))

vi.mock('./components/InsightsPage', () => ({
  InsightsPage: () => <div>Insights Page</div>,
}))

vi.mock('./pages/ActivityPage', () => ({
  ActivityPage: () => <div>Activity Page</div>,
}))

vi.mock('./pages/AnalyticsPage', () => ({
  AnalyticsPage: () => <div>Analytics Page</div>,
}))

vi.mock('./pages/ChatPageV2', () => ({
  ChatPageV2: () => <div>Chat Page</div>,
}))

vi.mock('./pages/LiveMonitorPage', () => ({
  LiveMonitorPage: () => <div>Live Monitor Page</div>,
}))

vi.mock('./pages/ReportsPage', () => ({
  ReportsPage: () => <div>Reports Page</div>,
}))

async function renderAt(path: string) {
  const { router } = await import('./router')
  const memoryRouter = createMemoryRouter(router.routes, {
    initialEntries: [path],
  })

  render(<RouterProvider router={memoryRouter} />)
  return memoryRouter
}

describe('router', () => {
  it('renders InsightsPage directly at /insights', async () => {
    const memoryRouter = await renderAt('/insights')

    expect(await screen.findByText('Insights Page')).toBeInTheDocument()
    expect(memoryRouter.state.location.pathname).toBe('/insights')
    expect(memoryRouter.state.location.search).toBe('')
  }, 10_000)

  it('keeps /analytics tab behavior unchanged', async () => {
    const memoryRouter = await renderAt('/analytics?tab=contributions')

    expect(await screen.findByText('Analytics Page')).toBeInTheDocument()
    expect(memoryRouter.state.location.pathname).toBe('/analytics')
    expect(memoryRouter.state.location.search).toBe('?tab=contributions')
  })
})
