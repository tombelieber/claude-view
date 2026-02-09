import { createBrowserRouter, Navigate, useParams } from 'react-router-dom'
import App from './App'
import { StatsDashboard } from './components/StatsDashboard'
import { HistoryView } from './components/HistoryView'
import { SearchResults } from './components/SearchResults'
import { ConversationView } from './components/ConversationView'
import { SettingsPage } from './components/SettingsPage'
import { ContributionsPage } from './pages/ContributionsPage'
import { sessionIdFromSlug } from './lib/url-slugs'

/** Redirect old /project/:projectId/session/:slug to flat /session/:sessionId */
function OldSessionRedirect() {
  const { slug } = useParams()
  const sessionId = slug ? sessionIdFromSlug(slug) : ''
  return <Navigate to={`/session/${sessionId}`} replace />
}

/** Redirect legacy /session/:projectId/:sessionId to flat /session/:sessionId */
function LegacySessionRedirect() {
  const { sessionId } = useParams()
  return <Navigate to={`/session/${sessionId}`} replace />
}

/** Redirect old /project/:projectId/contributions to flat /contributions?project=... */
function OldContributionsRedirect() {
  const { projectId } = useParams()
  const project = projectId ? decodeURIComponent(projectId) : ''
  return <Navigate to={`/contributions?project=${encodeURIComponent(project)}`} replace />
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
      { index: true, element: <StatsDashboard /> },
      { path: 'sessions', element: <HistoryView /> },
      { path: 'settings', element: <SettingsPage /> },
      // Flat session route (new canonical URL)
      { path: 'session/:sessionId', element: <ConversationView /> },
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
      // Flat contributions route (new canonical URL, uses ?project= query param)
      { path: 'contributions', element: <ContributionsPage /> },
      // Redirects for old URLs
      { path: 'history', element: <Navigate to="/sessions" replace /> },
      // Legacy redirect
      { path: 'session/:projectId/:sessionId', element: <LegacySessionRedirect /> },
    ],
  },
])
