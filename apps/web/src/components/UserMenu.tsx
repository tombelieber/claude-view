import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, Link2, LogOut, Settings } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useAuth } from '../hooks/use-auth'
import { supabase } from '../lib/supabase'

/** Deterministic color from user ID for initials avatar fallback */
const AVATAR_COLORS = [
  'bg-blue-600',
  'bg-emerald-600',
  'bg-violet-600',
  'bg-amber-600',
  'bg-rose-600',
  'bg-cyan-600',
  'bg-indigo-600',
  'bg-teal-600',
]

function hashCode(str: string): number {
  let hash = 0
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0
  }
  return Math.abs(hash)
}

function Avatar({ user }: { user: { avatarUrl?: string; email?: string; id: string } }) {
  if (user.avatarUrl) {
    return (
      <img
        src={user.avatarUrl}
        alt=""
        className="w-7 h-7 rounded-full object-cover"
        referrerPolicy="no-referrer"
      />
    )
  }

  const initial = (user.email?.[0] ?? '?').toUpperCase()
  const color = AVATAR_COLORS[hashCode(user.id) % AVATAR_COLORS.length]

  return (
    <div
      className={`w-7 h-7 rounded-full ${color} flex items-center justify-center text-white text-xs font-semibold`}
    >
      {initial}
    </div>
  )
}

export function UserMenu() {
  const { user, loading, signOut, openSignIn } = useAuth()

  // Don't render anything when Supabase isn't configured (dev without env vars)
  if (!supabase) return null

  if (loading) return null

  // Signed-out state: "Sign in" button
  if (!user) {
    return (
      <button
        type="button"
        onClick={() => openSignIn()}
        className="text-sm text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-200 cursor-pointer transition-colors duration-150 px-2 py-1.5 rounded-md focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1"
      >
        Sign in
      </button>
    )
  }

  // Signed-in state: avatar + dropdown
  const providerLabel =
    user.provider === 'google'
      ? 'Google'
      : user.provider === 'email'
        ? 'Email'
        : (user.provider ?? 'Email')

  return (
    <Popover.Root>
      <Popover.Trigger asChild>
        <button
          type="button"
          className="flex items-center gap-1.5 cursor-pointer rounded-full p-0.5 hover:ring-2 hover:ring-gray-200 dark:hover:ring-gray-700 transition-all duration-150 focus-visible:ring-2 focus-visible:ring-blue-400"
          aria-label="User menu"
        >
          <Avatar user={user} />
          <ChevronDown className="w-3 h-3 text-gray-400" aria-hidden="true" />
        </button>
      </Popover.Trigger>

      <Popover.Portal>
        <Popover.Content
          align="end"
          sideOffset={8}
          className="z-50 w-64 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg p-1 animate-in fade-in-0 zoom-in-95"
        >
          {/* User info header */}
          <div className="px-3 py-2.5 border-b border-gray-100 dark:border-gray-800">
            <p className="text-sm font-medium text-gray-900 dark:text-gray-100 truncate">
              {user.email ?? 'Unknown'}
            </p>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">via {providerLabel}</p>
          </div>

          {/* Menu items */}
          <div className="py-1">
            <Popover.Close asChild>
              <Link
                to="/settings#shared-links"
                className="flex items-center gap-2.5 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md cursor-pointer transition-colors"
              >
                <Link2 className="w-4 h-4 text-gray-400" />
                My Shares
              </Link>
            </Popover.Close>
            <Popover.Close asChild>
              <Link
                to="/settings"
                className="flex items-center gap-2.5 px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md cursor-pointer transition-colors"
              >
                <Settings className="w-4 h-4 text-gray-400" />
                Account Settings
              </Link>
            </Popover.Close>
          </div>

          {/* Sign out */}
          <div className="border-t border-gray-100 dark:border-gray-800 py-1">
            <Popover.Close asChild>
              <button
                type="button"
                onClick={signOut}
                className="flex items-center gap-2.5 w-full px-3 py-2 text-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md cursor-pointer transition-colors"
              >
                <LogOut className="w-4 h-4 text-gray-400" />
                Sign out
              </button>
            </Popover.Close>
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  )
}
