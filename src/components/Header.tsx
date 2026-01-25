import { Link, useLocation, useParams } from 'react-router-dom'
import { Home, Search, HelpCircle, Settings, ChevronRight } from 'lucide-react'
import { useAppStore } from '../store/app-store'

export function Header() {
  const location = useLocation()
  const params = useParams()
  const { openCommandPalette, searchQuery } = useAppStore()

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname.startsWith('/project/')) {
      crumbs.push({
        label: decodeURIComponent(params.projectId || '').split('/').pop() || 'Project',
        path: location.pathname
      })
    }

    if (location.pathname.startsWith('/session/')) {
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
        </Link>

        {breadcrumbs.map((crumb, i) => (
          <div key={crumb.path} className="flex items-center gap-2">
            <ChevronRight className="w-4 h-4 text-gray-300" />
            {i === breadcrumbs.length - 1 ? (
              <span className="text-sm text-gray-600 truncate max-w-[200px]">
                {crumb.label}
              </span>
            ) : (
              <Link
                to={crumb.path}
                className="text-sm text-gray-600 hover:text-gray-900 truncate max-w-[200px]"
              >
                {crumb.label}
              </Link>
            )}
          </div>
        ))}
      </div>

      {/* Right: Search + Actions */}
      <div className="flex items-center gap-2">
        <button
          onClick={openCommandPalette}
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
        >
          <Search className="w-4 h-4" />
          <span className="hidden sm:inline">Search</span>
          <kbd className="hidden sm:inline text-xs text-gray-400 bg-white px-1.5 py-0.5 rounded border border-gray-200">
            ⌘K
          </kbd>
        </button>

        <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
          <HelpCircle className="w-5 h-5" />
        </button>

        <button className="p-2 text-gray-400 hover:text-gray-600 transition-colors">
          <Settings className="w-5 h-5" />
        </button>
      </div>
    </header>
  )
}
