import { Link, useLocation, useParams, useSearchParams } from 'react-router-dom'
import { Home, Search, HelpCircle, Settings, ChevronRight, Sun, Moon, Monitor } from 'lucide-react'
import { useAppStore } from '../store/app-store'
import { useTheme } from '../hooks/use-theme'
import { HealthIndicator } from './HealthIndicator'

const THEME_LABELS = { light: 'Light', dark: 'Dark', system: 'System' } as const
const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor } as const

export function Header() {
  const location = useLocation()
  const params = useParams()
  const [searchParams] = useSearchParams()
  const { openCommandPalette, searchQuery } = useAppStore()
  const { theme, cycleTheme } = useTheme()
  const ThemeIcon = THEME_ICONS[theme]

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname === '/contributions') {
      const projectFilter = searchParams.get('project')
      if (projectFilter) {
        crumbs.push({
          label: projectFilter.split('/').pop() || 'Project',
          path: `/?project=${encodeURIComponent(projectFilter)}`
        })
      }
      crumbs.push({
        label: 'Contributions',
        path: location.pathname + location.search
      })
    } else if (location.pathname.match(/^\/session\/[^/]+$/)) {
      // Flat session page: /session/:sessionId
      crumbs.push({
        label: 'Session',
        path: location.pathname
      })
    } else if (location.pathname.startsWith('/project/')) {
      // Legacy project routes (still supported via router redirects)
      const projectName = decodeURIComponent(params.projectId || '')
      crumbs.push({
        label: projectName.split('/').pop() || 'Project',
        path: `/?project=${encodeURIComponent(projectName)}`
      })
    }

    if (location.pathname === '/search') {
      crumbs.push({ label: 'Search', path: '/search' })
    }

    if (location.pathname === '/sessions') {
      crumbs.push({ label: 'Sessions', path: '/sessions' })
    }

    if (location.pathname === '/settings') {
      crumbs.push({ label: 'Settings', path: '/settings' })
    }

    return crumbs
  }

  const breadcrumbs = getBreadcrumbs()

  return (
    <header className="h-12 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4">
      {/* Left: Logo + Breadcrumbs */}
      <div className="flex items-center gap-2">
        <Link
          to="/"
          className="flex items-center gap-2 hover:opacity-70 transition-opacity"
        >
          <Home className="w-4 h-4 text-gray-400" />
          <h1 className="text-lg font-semibold text-gray-900 dark:text-gray-100">Claude View</h1>
          <HealthIndicator />
        </Link>

        {breadcrumbs.length > 0 && (
          <nav aria-label="Breadcrumb" className="flex items-center">
            {breadcrumbs.map((crumb, i) => (
              <div key={crumb.path} className="flex items-center gap-2">
                <ChevronRight className="w-4 h-4 text-gray-300" aria-hidden="true" />
                {i === breadcrumbs.length - 1 ? (
                  <span className="text-sm text-gray-600 dark:text-gray-400 truncate max-w-[200px]" aria-current="page">
                    {crumb.label}
                  </span>
                ) : (
                  <Link
                    to={crumb.path}
                    className="text-sm text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-200 cursor-pointer truncate max-w-[200px] transition-colors duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-400 rounded-sm"
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
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 dark:text-gray-400 dark:hover:text-gray-200 dark:bg-gray-800 dark:hover:bg-gray-700 rounded-lg cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
          aria-label="Open search (Command K)"
        >
          <Search className="w-4 h-4" aria-hidden="true" />
          <span className="hidden sm:inline">Search</span>
          <kbd className="hidden sm:inline text-xs text-gray-400 bg-white dark:bg-gray-900 dark:border-gray-600 px-1.5 py-0.5 rounded border border-gray-200" aria-hidden="true">
            Cmd+K
          </kbd>
        </button>

        <button
          onClick={cycleTheme}
          aria-label={`Theme: ${THEME_LABELS[theme]}. Click to cycle.`}
          title={`Theme: ${THEME_LABELS[theme]}`}
          className="flex items-center gap-1.5 px-2.5 py-1.5 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <ThemeIcon className="w-4 h-4" aria-hidden="true" />
          <span className="text-xs font-medium">{THEME_LABELS[theme]}</span>
        </button>

        <button
          type="button"
          aria-label="Help"
          className="p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <HelpCircle className="w-5 h-5" aria-hidden="true" />
        </button>

        <Link
          to="/settings"
          aria-label="Settings"
          className="p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <Settings className="w-5 h-5" aria-hidden="true" />
        </Link>
      </nav>
    </header>
  )
}
