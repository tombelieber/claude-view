import { Navigate, createBrowserRouter, useParams } from 'react-router-dom'
import App from './App'
import { ConversationView } from './components/ConversationView'
import { HistoryView } from './components/HistoryView'
import { InsightsPage } from './components/InsightsPage'
import { SearchResults } from './components/SearchResults'
import { SettingsPage } from './components/SettingsPage'
import { sessionIdFromSlug } from './lib/url-slugs'
import { ActivityPage } from './pages/ActivityPage'
import { AnalyticsPage } from './pages/AnalyticsPage'
import { LiveMonitorPage } from './pages/LiveMonitorPage'
import { PluginsPage } from './pages/PluginsPage'
import { PromptsPage } from './pages/PromptsPage'
import { ReportsPage } from './pages/ReportsPage'
import { SystemMonitorPage } from './pages/SystemMonitorPage'
import { TeamsPage } from './pages/TeamsPage'
import { WorkflowDetailPage } from './pages/WorkflowDetailPage'
import { WorkflowsPage } from './pages/WorkflowsPage'

/** Redirect old /project/:projectId/session/:slug to flat /sessions/:sessionId */
function OldSessionRedirect() {
  const { slug } = useParams()
  const sessionId = slug ? sessionIdFromSlug(slug) : ''
  return <Navigate to={`/sessions/${sessionId}`} replace />
}

/** Redirect legacy /session/:projectId/:sessionId to flat /sessions/:sessionId */
function LegacySessionRedirect() {
  const { sessionId } = useParams()
  return <Navigate to={`/sessions/${sessionId}`} replace />
}

/** Redirect old singular /session/:sessionId to new /sessions/:sessionId */
function SingularSessionRedirect() {
  const { sessionId } = useParams()
  return <Navigate to={`/sessions/${sessionId}`} replace />
}

/** Redirect old /project/:projectId/contributions to /analytics?tab=contributions&project=... */
function OldContributionsRedirect() {
  const { projectId } = useParams()
  const project = projectId ? decodeURIComponent(projectId) : ''
  return (
    <Navigate to={`/analytics?tab=contributions&project=${encodeURIComponent(project)}`} replace />
  )
}

/** Redirect old /project/:projectId to flat /?project=... */
function OldProjectRedirect() {
  const { projectId } = useParams()
  const project = projectId ? decodeURIComponent(projectId) : ''
  return <Navigate to={`/?project=${encodeURIComponent(project)}`} replace />
}

export const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <LiveMonitorPage /> },
      { path: 'sessions', element: <HistoryView /> },
      { path: 'sessions/:sessionId', element: <ConversationView /> },
      { path: 'analytics', element: <AnalyticsPage /> },
      { path: 'activity', element: <ActivityPage /> },
      { path: 'reports', element: <ReportsPage /> },
      { path: 'prompts', element: <PromptsPage /> },
      { path: 'teams', element: <TeamsPage /> },
      { path: 'workflows', element: <WorkflowsPage /> },
      { path: 'workflows/:id', element: <WorkflowDetailPage /> },
      { path: 'plugins', element: <PluginsPage /> },
      { path: 'monitor', element: <SystemMonitorPage /> },
      { path: 'settings', element: <SettingsPage /> },
      { path: 'system', element: <Navigate to="/settings" replace /> },
      {
        path: 'project/:projectId',
        children: [
          { index: true, element: <OldProjectRedirect /> },
          { path: 'contributions', element: <OldContributionsRedirect /> },
          // Redirect old nested session URLs to flat structure
          { path: 'session/:slug', element: <OldSessionRedirect /> },
        ],
      },
      { path: 'search', element: <SearchResults /> },
      // Redirect old contributions URL to analytics tab
      { path: 'contributions', element: <Navigate to="/analytics?tab=contributions" replace /> },
      { path: 'mission-control', element: <Navigate to="/" replace /> },
      // Redirects for old URLs
      { path: 'history', element: <Navigate to="/sessions" replace /> },
      { path: 'insights', element: <InsightsPage /> },
      // Redirect old singular /session/:id to /sessions/:id
      { path: 'session/:sessionId', element: <SingularSessionRedirect /> },
      // Legacy redirect
      { path: 'session/:projectId/:sessionId', element: <LegacySessionRedirect /> },
    ],
  },
])
