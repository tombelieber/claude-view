import {
  ChevronRight,
  ExternalLink,
  Github,
  HelpCircle,
  Home,
  Keyboard,
  MessageSquarePlus,
  Monitor,
  Moon,
  Search,
  Settings,
  Sun,
  Tag,
} from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useLocation, useNavigate, useParams, useSearchParams } from 'react-router-dom'
import type { NotificationSoundSettings } from '../hooks/use-notification-sound'
import { useTheme } from '../hooks/use-theme'
import { useTrackEvent } from '../hooks/use-track-event'
import { useAppStore } from '../store/app-store'

const GITHUB_URL = 'https://github.com/tombelieber/claude-view'

interface HelpItem {
  icon: React.ReactNode
  label: string
  href?: string
  onClick?: () => void
  external?: boolean
}

function HelpMenu({ onClose, onNavigate }: { onClose: () => void; onNavigate: (path: string) => void }) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose()
    }
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    document.addEventListener('mousedown', handleClickOutside)
    document.addEventListener('keydown', handleEscape)
    return () => {
      document.removeEventListener('mousedown', handleClickOutside)
      document.removeEventListener('keydown', handleEscape)
    }
  }, [onClose])

  const items: HelpItem[] = [
    {
      icon: <MessageSquarePlus className="w-4 h-4" />,
      label: 'Feedback & Issues',
      href: `${GITHUB_URL}/issues/new`,
      external: true,
    },
    {
      icon: <Keyboard className="w-4 h-4" />,
      label: 'Keyboard Shortcuts',
      onClick: () => {
        onNavigate('/settings')
        onClose()
        // Defer scroll until Settings page mounts
        setTimeout(() => {
          document.getElementById('keyboard-shortcuts')?.scrollIntoView({ behavior: 'smooth', block: 'start' })
        }, 100)
      },
    },
    {
      icon: <Tag className="w-4 h-4" />,
      label: 'Release Notes',
      href: `${GITHUB_URL}/releases`,
      external: true,
    },
    {
      icon: <Github className="w-4 h-4" />,
      label: 'GitHub',
      href: GITHUB_URL,
      external: true,
    },
  ]

  return (
    <div
      ref={ref}
      className="absolute right-0 top-full mt-1 z-50 w-52 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-lg py-1 animate-in fade-in slide-in-from-top-1 duration-150"
    >
      {items.map((item) => {
        const cls =
          'flex w-full items-center gap-2.5 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer'

        return item.href ? (
          <a
            key={item.label}
            href={item.href}
            target="_blank"
            rel="noopener noreferrer"
            className={cls}
            onClick={onClose}
          >
            <span className="text-gray-400 dark:text-gray-500">{item.icon}</span>
            <span className="flex-1">{item.label}</span>
            {item.external && <ExternalLink className="w-3 h-3 text-gray-300 dark:text-gray-600" />}
          </a>
        ) : (
          <button key={item.label} type="button" className={cls} onClick={item.onClick}>
            <span className="text-gray-400 dark:text-gray-500">{item.icon}</span>
            <span className="flex-1 text-left">{item.label}</span>
          </button>
        )
      })}
    </div>
  )
}
import { AuthPill } from './AuthPill'
import { HealthIndicator } from './HealthIndicator'
import { UserMenu } from './UserMenu'
import { NotificationSoundPopover } from './live/NotificationSoundPopover'

const THEME_LABELS = { light: 'Light', dark: 'Dark', system: 'System' } as const
const THEME_ICONS = { light: Sun, dark: Moon, system: Monitor } as const

interface HeaderProps {
  soundSettings: NotificationSoundSettings
  onSoundSettingsChange: (patch: Partial<NotificationSoundSettings>) => void
  onSoundPreview: () => void
  audioUnlocked: boolean
}

export function Header({
  soundSettings,
  onSoundSettingsChange,
  onSoundPreview,
  audioUnlocked,
}: HeaderProps) {
  const location = useLocation()
  const navigate = useNavigate()
  const params = useParams()
  const [searchParams] = useSearchParams()
  const { openCommandPalette } = useAppStore()
  const { theme, cycleTheme } = useTheme()
  const trackEvent = useTrackEvent()
  const [helpOpen, setHelpOpen] = useState(false)
  const toggleHelp = useCallback(() => setHelpOpen((v) => !v), [])
  const THEME_CYCLE = ['light', 'dark', 'system'] as const
  const handleCycleTheme = () => {
    const idx = THEME_CYCLE.indexOf(theme)
    const nextTheme = THEME_CYCLE[(idx + 1) % THEME_CYCLE.length]
    cycleTheme()
    trackEvent('theme_toggled', { to: nextTheme })
  }
  const ThemeIcon = THEME_ICONS[theme]

  // Build breadcrumbs based on current route
  const getBreadcrumbs = () => {
    const crumbs: { label: string; path: string }[] = []

    if (location.pathname === '/analytics') {
      const tab = searchParams.get('tab')
      const tabLabel =
        tab === 'contributions' ? 'Contributions' : tab === 'insights' ? 'Insights' : 'Overview'
      crumbs.push({
        label: `Analytics — ${tabLabel}`,
        path: location.pathname + location.search,
      })
    } else if (location.pathname.match(/^\/sessions\/[^/]+$/)) {
      // Session detail page: /sessions/:sessionId
      crumbs.push({
        label: 'Session',
        path: location.pathname,
      })
    } else if (location.pathname.startsWith('/project/')) {
      // Legacy project routes (still supported via router redirects)
      const projectName = decodeURIComponent(params.projectId || '')
      crumbs.push({
        label: projectName.split('/').pop() || 'Project',
        path: `/?project=${encodeURIComponent(projectName)}`,
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

    if (location.pathname === '/teams') {
      crumbs.push({ label: 'Teams', path: '/teams' })
    }

    return crumbs
  }

  const breadcrumbs = getBreadcrumbs()

  return (
    <header className="h-12 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between px-4">
      {/* Left: Logo + Breadcrumbs */}
      <div className="flex items-center gap-2">
        <Link to="/" className="flex items-center gap-2 hover:opacity-70 transition-opacity">
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
                  <span
                    className="text-sm text-gray-600 dark:text-gray-400 truncate max-w-[200px]"
                    aria-current="page"
                  >
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
            <Search
              className="absolute left-3 w-4 h-4 text-gray-400 pointer-events-none"
              aria-hidden="true"
            />
            <input
              type="text"
              placeholder="Search sessions..."
              className="w-full pl-9 pr-12 py-1.5 text-sm bg-gray-100 dark:bg-gray-800 border border-transparent hover:border-gray-300 dark:hover:border-gray-600 focus:border-blue-500 dark:focus:border-blue-400 focus:bg-white dark:focus:bg-gray-900 rounded-lg outline-none transition-colors text-gray-900 dark:text-gray-100 placeholder-gray-500 dark:placeholder-gray-400 cursor-pointer"
              onFocus={() => openCommandPalette()}
              readOnly
              aria-label="Search sessions (Command K)"
            />
            <kbd
              className="absolute right-3 text-xs text-gray-400 bg-white dark:bg-gray-900 dark:border-gray-600 px-1.5 py-0.5 rounded border border-gray-200 pointer-events-none"
              aria-hidden="true"
            >
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
          onClick={handleCycleTheme}
          aria-label={`Theme: ${THEME_LABELS[theme]}. Click to cycle.`}
          title={`Theme: ${THEME_LABELS[theme]}`}
          className="flex items-center gap-1.5 px-2.5 py-1.5 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <ThemeIcon className="w-4 h-4" aria-hidden="true" />
          <span className="text-xs font-medium">{THEME_LABELS[theme]}</span>
        </button>

        <UserMenu />

        <div className="relative">
          <button
            type="button"
            aria-label="Help"
            aria-expanded={helpOpen}
            onClick={toggleHelp}
            className="p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
          >
            <HelpCircle className="w-5 h-5" aria-hidden="true" />
          </button>
          {helpOpen && <HelpMenu onClose={() => setHelpOpen(false)} onNavigate={navigate} />}
        </div>

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
