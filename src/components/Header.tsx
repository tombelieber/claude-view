import { Link, useLocation, useParams, useSearchParams } from 'react-router-dom'
import { Home, Search, HelpCircle, Settings, ChevronRight, Sun, Moon, Monitor } from 'lucide-react'
import { useAppStore } from '../store/app-store'
import { useTheme } from '../hooks/use-theme'
import { HealthIndicator } from './HealthIndicator'
import { AuthPill } from './AuthPill'
import { NotificationSoundPopover } from './live/NotificationSoundPopover'
import type { NotificationSoundSettings } from '../hooks/use-notification-sound'

const THEME_LABELS = { light: 'Light', dark: 'Dark', system: 'System' } as const
const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor } as const

interface HeaderProps {
  soundSettings: NotificationSoundSettings
  onSoundSettingsChange: (patch: Partial<NotificationSoundSettings>) => void
  onSoundPreview: () => void
  audioUnlocked: boolean
}

export function Header({ soundSettings, onSoundSettingsChange, onSoundPreview, audioUnlocked }: HeaderProps) {
  const location = useLocation()
  const params = useParams()
  const [searchParams] = useSearchParams()
  const { openCommandPalette } = useAppStore()
  const { theme, cycleTheme } = useTheme()
  const ThemeIcon = THEME_ICONS[theme]

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname === '/analytics') {
      const tab = searchParams.get('tab')
      const tabLabel = tab === 'contributions' ? 'Contributions'
        : tab === 'insights' ? 'Insights'
        : 'Overview'
      crumbs.push({
        label: `Analytics — ${tabLabel}`,
        path: location.pathname + location.search
      })
    } else if (location.pathname.match(/^\/sessions\/[^/]+$/)) {
      // Session detail page: /sessions/:sessionId
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
        <AuthPill />

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
        {/* Search bar - clicking opens CommandPalette */}
        <div className="relative flex-1 max-w-md mx-4">
          <div className="flex items-center">
            <Search className="absolute left-3 w-4 h-4 text-gray-400 pointer-events-none" aria-hidden="true" />
            <input
              type="text"
              placeholder="Search sessions..."
              className="w-full pl-9 pr-12 py-1.5 text-sm bg-gray-100 dark:bg-gray-800 border border-transparent hover:border-gray-300 dark:hover:border-gray-600 focus:border-blue-500 dark:focus:border-blue-400 focus:bg-white dark:focus:bg-gray-900 rounded-lg outline-none transition-colors text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 cursor-pointer"
              onFocus={() => openCommandPalette()}
              readOnly
              aria-label="Search sessions (Command K)"
            />
            <kbd className="absolute right-3 text-xs text-gray-400 bg-white dark:bg-gray-900 dark:border-gray-600 px-1.5 py-0.5 rounded border border-gray-200 pointer-events-none" aria-hidden="true">
              &#8984;K
            </kbd>
          </div>
        </div>

        <NotificationSoundPopover
          settings={soundSettings}
          onSettingsChange={onSoundSettingsChange}
          onPreview={onSoundPreview}
          audioUnlocked={audioUnlocked}
        />

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
