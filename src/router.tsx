import { createBrowserRouter } from 'react-router-dom'
import App from './App'
import { StatsDashboard } from './components/StatsDashboard'
import { ProjectView } from './components/ProjectView'
import { HistoryView } from './components/HistoryView'
import { SearchResults } from './components/SearchResults'
import { ConversationView } from './components/ConversationView'

export const router = createBrowserRouter([
  {
    path: '/',
    element: <App />,
    children: [
      { index: true, element: <StatsDashboard /> },
      { path: 'history', element: <HistoryView /> },
      { path: 'project/:projectId', element: <ProjectView /> },
      { path: 'search', element: <SearchResults /> },
      { path: 'session/:projectId/:sessionId', element: <ConversationView /> },
    ],
  },
])
