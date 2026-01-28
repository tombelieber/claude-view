import { Link, useLocation, useParams } from 'react-router-dom'
import { Home, Search, HelpCircle, Settings, ChevronRight } from 'lucide-react'
import { useAppStore } from '../store/app-store'
import { HealthIndicator } from './HealthIndicator'

export function Header() {
  const location = useLocation()
  const params = useParams()
  const { openCommandPalette, searchQuery } = useAppStore()

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname.startsWith('/project/') && params.slug) {
      // Session page: /project/:projectId/session/:slug
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: `/project/${params.projectId}`
      })
      crumbs.push({
        label: 'Session',
        path: location.pathname
      })
    } else if (location.pathname.startsWith('/project/')) {
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: location.pathname
      })
    }

    if (location.pathname.startsWith('/session/')) {
      // Legacy URL — redirect will handle it, but show breadcrumbs in the meantime
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: `/project/${params.projectId}`
      })
      crumbs.push({
        label: 'Session',
        path: location.pathname
      })
    }

    if (location.pathname === '/search') {
      crumbs.push({ label: 'Search', path: '/search' })
    }

    if (location.pathname === '/history') {
      crumbs.push({ label: 'History', path: '/history' })
    }

    if (location.pathname === '/settings') {
      crumbs.push({ label: 'Settings', path: '/settings' })
    }

    return crumbs
  }

  const breadcrumbs = getBreadcrumbs()

  return (
    <header className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-4">
      {/* Left: Logo + Breadcrumbs */}
      <div className="flex items-center gap-2">
        <Link
          to="/"
          className="flex items-center gap-2 hover:opacity-70 transition-opacity"
        >
          <Home className="w-4 h-4 text-gray-400" />
          <h1 className="text-lg font-semibold text-gray-900">Claude View</h1>
          <HealthIndicator />
        </Link>

        {breadcrumbs.length > 0 && (
          <nav aria-label="Breadcrumb" className="flex items-center">
            {breadcrumbs.map((crumb, i) => (
              <div key={crumb.path} className="flex items-center gap-2">
                <ChevronRight className="w-4 h-4 text-gray-300" aria-hidden="true" />
                {i === breadcrumbs.length - 1 ? (
                  <span className="text-sm text-gray-600 truncate max-w-[200px]" aria-current="page">
                    {crumb.label}
                  </span>
                ) : (
                  <Link
                    to={crumb.path}
                    className="text-sm text-gray-600 hover:text-gray-900 cursor-pointer truncate max-w-[200px] transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 rounded-sm"
                  >
                    {crumb.label}
                  </Link>
                )}
              </div>
            ))}
          </nav>
        )}
      </div>

      {/* Right: Search + Actions */}
      <nav className="flex items-center gap-2" aria-label="Main actions">
        <button
          onClick={openCommandPalette}
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          aria-label="Open search (Command K)"
        >
          <Search className="w-4 h-4" aria-hidden="true" />
          <span className="hidden sm:inline">Search</span>
          <kbd className="hidden sm:inline text-xs text-gray-400 bg-white px-1.5 py-0.5 rounded border border-gray-200" aria-hidden="true">
            Cmd+K
          </kbd>
        </button>

        <button
          type="button"
          aria-label="Help"
          className="p-2 text-gray-400 hover:text-gray-600 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <HelpCircle className="w-5 h-5" aria-hidden="true" />
        </button>

        <Link
          to="/settings"
          aria-label="Settings"
          className="p-2 text-gray-400 hover:text-gray-600 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <Settings className="w-5 h-5" aria-hidden="true" />
        </Link>
      </nav>
    </header>
  )
}
