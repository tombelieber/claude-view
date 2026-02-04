import { createBrowserRouter, Navigate, useParams } from 'react-router-dom'
import App from './App'
import { StatsDashboard } from './components/StatsDashboard'
import { ProjectView } from './components/ProjectView'
import { HistoryView } from './components/HistoryView'
import { SearchResults } from './components/SearchResults'
import { ConversationView } from './components/ConversationView'
import { SettingsPage } from './components/SettingsPage'
import { ContributionsPage } from './pages/ContributionsPage'

/** Redirect legacy /session/:projectId/:sessionId to /project/:projectId/session/:sessionId */
function LegacySessionRedirect() {
  const { projectId, sessionId } = useParams()
  return <Navigate to={`/project/${projectId}/session/${sessionId}`} replace />
}

export const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <StatsDashboard /> },
      { path: 'contributions', element: <ContributionsPage /> },
      { path: 'history', element: <HistoryView /> },
      { path: 'settings', element: <SettingsPage /> },
      { path: 'project/:projectId', element: <ProjectView /> },
      { path: 'project/:projectId/session/:slug', element: <ConversationView /> },
      { path: 'search', element: <SearchResults /> },
      // Legacy redirect
      { path: 'session/:projectId/:sessionId', element: <LegacySessionRedirect /> },
    ],
  },
])
