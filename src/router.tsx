import { createBrowserRouter, Navigate, useParams } from 'react-router-dom'
import App from './App'
import { HistoryView } from './components/HistoryView'
import { SearchResults } from './components/SearchResults'
import { ConversationView } from './components/ConversationView'
import { SettingsPage } from './components/SettingsPage'
import { LiveMonitorPage } from './pages/LiveMonitorPage'
import { MobileLayout } from './pages/MobileLayout'
import { MobilePairingPage } from './pages/MobilePairingPage'
import { MobileMonitorPageMobile } from './pages/MobileMonitorPage'
import { AnalyticsPage } from './pages/AnalyticsPage'
import { ReportsPage } from './pages/ReportsPage'
import { sessionIdFromSlug } from './lib/url-slugs'

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
  return <Navigate to={`/analytics?tab=contributions&project=${encodeURIComponent(project)}`} replace />
}

/** Redirect old /project/:projectId to flat /?project=... */
function OldProjectRedirect() {
  const { projectId } = useParams()
  const project = projectId ? decodeURIComponent(projectId) : ''
  return <Navigate to={`/?project=${encodeURIComponent(project)}`} replace />
}

export const router = createBrowserRouter([
  {
    path: '/mobile',
    element: <MobileLayout />,
    children: [
      { index: true, element: <MobilePairingPage /> },
      { path: 'monitor', element: <MobileMonitorPageMobile /> },
    ],
  },
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <LiveMonitorPage /> },
      { path: 'sessions', element: <HistoryView /> },
      { path: 'sessions/:sessionId', element: <ConversationView /> },
      { path: 'analytics', element: <AnalyticsPage /> },
      { path: 'reports', element: <ReportsPage /> },
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
      { path: 'insights', element: <Navigate to="/analytics?tab=insights" replace /> },
      // Redirect old singular /session/:id to /sessions/:id
      { path: 'session/:sessionId', element: <SingularSessionRedirect /> },
      // Legacy redirect
      { path: 'session/:projectId/:sessionId', element: <LegacySessionRedirect /> },
    ],
  },
])
