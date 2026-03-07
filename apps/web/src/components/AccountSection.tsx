// apps/web/src/components/AccountSection.tsx
import { LogOut, User } from 'lucide-react'
import { useAuth } from '../hooks/use-auth'
import { useConfig } from '../hooks/use-config'

export function AccountSection() {
  const { user, loading, signOut, openSignIn } = useAuth()
  const { auth } = useConfig()

  // Don't render if auth isn't configured (local mode)
  if (!auth) return null

  if (loading) return null

  // Signed-out state
  if (!user) {
    return (
      <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
        <div className="flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
          <span className="text-gray-500 dark:text-gray-400">
            <User className="w-4 h-4" />
          </span>
          <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
            Account
          </h2>
        </div>
        <div className="p-4">
          <p className="text-sm text-gray-500 dark:text-gray-400 mb-3">
            Sign in to enable sharing and sync.
          </p>
          <button
            type="button"
            onClick={() => openSignIn()}
            className="inline-flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-md cursor-pointer transition-colors duration-150 bg-gray-900 dark:bg-gray-100 text-white dark:text-gray-900 hover:bg-gray-800 dark:hover:bg-gray-200 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2"
          >
            Sign in
          </button>
        </div>
      </div>
    )
  }

  // Signed-in state
  const providerLabel =
    user.provider === 'google'
      ? 'Google'
      : user.provider === 'email'
        ? 'Email'
        : (user.provider ?? 'Email')

  return (
    <div className="bg-white dark:bg-gray-900 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div className="flex items-center gap-2 px-4 py-3 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <span className="text-gray-500 dark:text-gray-400">
          <User className="w-4 h-4" />
        </span>
        <h2 className="text-sm font-semibold text-gray-700 dark:text-gray-300 uppercase tracking-wide">
          Account
        </h2>
      </div>
      <div className="p-4">
        <div className="flex items-center gap-4">
          {/* Avatar */}
          {user.avatarUrl ? (
            <img
              src={user.avatarUrl}
              alt=""
              className="w-10 h-10 rounded-full object-cover flex-shrink-0"
              referrerPolicy="no-referrer"
            />
          ) : (
            <div className="w-10 h-10 rounded-full bg-gray-200 dark:bg-gray-700 flex items-center justify-center flex-shrink-0">
              <User className="w-5 h-5 text-gray-500 dark:text-gray-400" />
            </div>
          )}

          {/* User info */}
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
              {user.email ?? 'Unknown'}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
              Signed in via {providerLabel}
            </p>
          </div>
        </div>

        <div className="mt-4 pt-4 border-t border-gray-100 dark:border-gray-800">
          <button
            type="button"
            onClick={signOut}
            className="inline-flex items-center gap-2 px-3 py-1.5 text-sm font-medium text-gray-700 dark:text-gray-300 border border-gray-200 dark:border-gray-700 rounded-md hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors cursor-pointer focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-2"
          >
            <LogOut className="w-4 h-4" />
            Sign out
          </button>
        </div>
      </div>
    </div>
  )
}
