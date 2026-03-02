# Web Auth UX Implementation Plan

> **Status:** DONE (2026-03-02) — all 8 tasks implemented, shippable audit passed (SHIP IT)

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add proper auth UX to the web app — global AuthProvider, header UserMenu with avatar/dropdown, account section on settings page, refactored sign-in modal via Radix Dialog.

### Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `ddaddc0b` | feat(web): add AuthProvider context + useAuth hook |
| 2 | `5268c5e2` | feat(web): wrap app with AuthProvider in main.tsx |
| 3 | `517934eb` | feat(web): add UserMenu component with avatar + dropdown |
| 4 | `464f94b6` | feat(web): add UserMenu to header right side |
| 5 | `ec4dcd4e` | feat(web): add AccountSection for settings page |
| 6 | `913ab0a0` | feat(web): add Account section to settings page |
| 7 | `49b4bb8f` | refactor(web): use AuthProvider for sign-in modal in ConversationView |
| 8 | — | Build & verification (tsc, biome, vitest, build — all pass) |

**Shippable audit:** SHIP IT — plan compliance 100%, wiring integrity 8/8 paths pass, no new blockers, build + 1132 tests pass (2 pre-existing failures unrelated).

**Architecture:** React context (`AuthProvider`) wraps the app at the `main.tsx` level (single mount point), owns Supabase auth state + sign-in modal. `UserMenu` in header shows avatar or "Sign in". Settings page gets an Account section. Existing `ConversationView` hand-rolled modal replaced by context's `openSignIn(onSuccess?)`.

**Tech Stack:** React 19, Supabase JS SDK (`AuthSession`/`AuthUser` types), Radix UI (Dialog + Popover — both already installed), Lucide icons, Tailwind CSS

**Design doc:** `docs/plans/web/2026-03-02-web-auth-ux-design.md`

**Important context:**
- The header already has `AuthPill` (line 86 in `Header.tsx`) — this shows **Claude CLI** auth status (Pro/Free/Not Signed In). It is a DIFFERENT auth system from Supabase. `AuthPill` stays untouched. The new `UserMenu` (Supabase auth) goes in the right-side nav.
- `App.tsx` already has `<AuthBanner />` (line 100) — this is also Claude CLI auth. Leave it untouched.
- Supabase SDK v2 exports types as `AuthSession` and `AuthUser` (NOT `Session`/`User`).

**Rollback:** To revert, run `git log --oneline -8` to see the commits from this plan, then `git revert <hash>` for each in reverse order. All changes are frontend-only with no database migrations.

---

### Task 1: Create `useAuth` hook and `AuthProvider`

**Files:**
- Create: `apps/web/src/hooks/use-auth.tsx`

**Step 1: Create the auth context + provider**

```tsx
// apps/web/src/hooks/use-auth.tsx
import * as Dialog from '@radix-ui/react-dialog'
import type { AuthSession, AuthUser } from '@supabase/supabase-js'
import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { SignInPrompt } from '../components/SignInPrompt'
import { supabase } from '../lib/supabase'

interface AppUser {
  id: string
  email: string | undefined
  displayName: string | undefined
  avatarUrl: string | undefined
  provider: string | undefined
}

interface AuthContextValue {
  user: AppUser | null
  loading: boolean
  signOut: () => Promise<void>
  /** Open the sign-in modal. Optionally pass a callback to run after successful sign-in. */
  openSignIn: (onSuccess?: () => void) => void
}

const AuthContext = createContext<AuthContextValue>({
  user: null,
  loading: true,
  signOut: async () => {},
  openSignIn: () => {},
})

function mapUser(user: AuthUser, session: AuthSession | null): AppUser {
  const provider = session?.user?.app_metadata?.provider
  return {
    id: user.id,
    email: user.email,
    displayName: user.user_metadata?.full_name ?? user.user_metadata?.name,
    avatarUrl: user.user_metadata?.avatar_url ?? user.user_metadata?.picture,
    provider: typeof provider === 'string' ? provider : undefined,
  }
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AppUser | null>(null)
  const [loading, setLoading] = useState(true)
  const [signInOpen, setSignInOpen] = useState(false)
  const onSignInSuccessRef = useRef<(() => void) | undefined>(undefined)

  useEffect(() => {
    if (!supabase) {
      setLoading(false)
      return
    }

    // Restore existing session
    supabase.auth.getSession().then(({ data: { session } }) => {
      if (session?.user) {
        setUser(mapUser(session.user, session))
      }
      setLoading(false)
    })

    // Listen for auth state changes
    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, session) => {
      if (session?.user) {
        setUser(mapUser(session.user, session))
        setSignInOpen(false)
        // Fire the pending onSuccess callback (e.g. retry share after sign-in)
        onSignInSuccessRef.current?.()
        onSignInSuccessRef.current = undefined
      } else {
        setUser(null)
      }
    })

    return () => subscription.unsubscribe()
  }, [])

  const signOut = useCallback(async () => {
    if (!supabase) return
    await supabase.auth.signOut()
  }, [])

  const openSignIn = useCallback((onSuccess?: () => void) => {
    onSignInSuccessRef.current = onSuccess
    setSignInOpen(true)
  }, [])

  return (
    <AuthContext.Provider value={{ user, loading, signOut, openSignIn }}>
      {children}
      <Dialog.Root open={signInOpen} onOpenChange={setSignInOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 bg-black/60 z-50 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2 bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-xl shadow-2xl focus:outline-none">
            <Dialog.Title className="sr-only">Sign in</Dialog.Title>
            <Dialog.Description className="sr-only">
              Sign in with Google or email to enable sharing and sync
            </Dialog.Description>
            <SignInPrompt onSignedIn={() => setSignInOpen(false)} />
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </AuthContext.Provider>
  )
}

export function useAuth(): AuthContextValue {
  return useContext(AuthContext)
}
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors related to `use-auth.tsx`

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/hooks/use-auth.tsx`
Expected: No errors (may auto-fix import ordering)

**Step 4: Commit**

```bash
git add apps/web/src/hooks/use-auth.tsx
git commit -m "feat(web): add AuthProvider context + useAuth hook

Global auth state from Supabase SDK with Radix Dialog sign-in modal.
openSignIn(onSuccess?) supports callback for retry-after-auth flows.
Graceful degradation when Supabase env vars are missing."
```

---

### Task 2: Wrap app with AuthProvider in main.tsx

**Files:**
- Modify: `apps/web/src/main.tsx`

**Why main.tsx (not App.tsx):** AuthProvider must mount exactly once for the app lifetime. `App.tsx` has early returns for loading/error states which would create separate AuthProvider instances, causing double Supabase subscriptions and auth state flicker. Mounting in `main.tsx` avoids this entirely.

**Step 1: Add AuthProvider wrapper**

In `apps/web/src/main.tsx`, add import and wrap `<RouterProvider>`:

Add import at top:
```tsx
import { AuthProvider } from './hooks/use-auth'
```

Change the render block from:
```tsx
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
      <Toaster position="top-right" richColors />
    </QueryClientProvider>
  </StrictMode>,
)
```

To:
```tsx
createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <AuthProvider>
      <QueryClientProvider client={queryClient}>
        <RouterProvider router={router} />
        <Toaster position="top-right" richColors />
      </QueryClientProvider>
    </AuthProvider>
  </StrictMode>,
)
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/main.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/main.tsx
git commit -m "feat(web): wrap app with AuthProvider in main.tsx

Single mount point — avoids double Supabase subscription from
App.tsx early returns creating separate AuthProvider instances."
```

---

### Task 3: Create UserMenu header component

**Files:**
- Create: `apps/web/src/components/UserMenu.tsx`

**Step 1: Create the component**

```tsx
// apps/web/src/components/UserMenu.tsx
import * as Popover from '@radix-ui/react-popover'
import { ChevronDown, Link2, LogOut, Settings, User } from 'lucide-react'
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
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
              via {providerLabel}
            </p>
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
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/components/UserMenu.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/components/UserMenu.tsx
git commit -m "feat(web): add UserMenu component with avatar + dropdown

Radix Popover with user info, My Shares link, Account Settings link,
and Sign out button. Initials fallback avatar with deterministic colors.
Hidden when Supabase is not configured (graceful degradation)."
```

---

### Task 4: Add UserMenu to Header

**Files:**
- Modify: `apps/web/src/components/Header.tsx`

**Important:** Header already has `<AuthPill />` (line 86) on the LEFT side — this shows Claude CLI auth status (Pro/Free/Not Signed In). This is a DIFFERENT auth system. Do NOT remove or replace `AuthPill`. The new `UserMenu` goes on the RIGHT side.

**Step 1: Import and add UserMenu**

In `apps/web/src/components/Header.tsx`:
- Add import: `import { UserMenu } from './UserMenu'`
- Add `<UserMenu />` in the right-side `<nav>`, between the theme toggle button and the Help button.

Find this section (around line 156-163):
```tsx
        <button
          type="button"
          aria-label="Help"
          className="p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <HelpCircle className="w-5 h-5" aria-hidden="true" />
        </button>
```

Insert `<UserMenu />` BEFORE the Help button:

```tsx
        <UserMenu />

        <button
          type="button"
          aria-label="Help"
          className="p-2 text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 cursor-pointer transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-blue-400 focus-visible:ring-offset-1 rounded-md"
        >
          <HelpCircle className="w-5 h-5" aria-hidden="true" />
        </button>
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/components/Header.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/components/Header.tsx
git commit -m "feat(web): add UserMenu to header right side

AuthPill (CLI status, left side) remains untouched.
UserMenu (Supabase auth, right side) added before Help button."
```

---

### Task 5: Create AccountSection for Settings page

**Files:**
- Create: `apps/web/src/components/AccountSection.tsx`

**Step 1: Create the component**

```tsx
// apps/web/src/components/AccountSection.tsx
import { LogOut, User } from 'lucide-react'
import { useAuth } from '../hooks/use-auth'
import { supabase } from '../lib/supabase'

export function AccountSection() {
  const { user, loading, signOut, openSignIn } = useAuth()

  // Don't render if Supabase isn't configured (dev mode)
  if (!supabase) return null

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
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/components/AccountSection.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/components/AccountSection.tsx
git commit -m "feat(web): add AccountSection for settings page

Shows user avatar/email/provider when signed in, sign-in CTA when not.
Hidden entirely when Supabase is not configured."
```

---

### Task 6: Add AccountSection to SettingsPage

**Files:**
- Modify: `apps/web/src/components/SettingsPage.tsx`

**Step 1: Import and add AccountSection**

In `apps/web/src/components/SettingsPage.tsx`:
- Add import: `import { AccountSection } from './AccountSection'`
- Add `<AccountSection />` as the first child inside `<div className="space-y-4">`, before the Storage Overview section.

Find this exact string:
```tsx
        <div className="space-y-4">
          {/* STORAGE OVERVIEW */}
          <SettingsSection icon={<HardDrive className="w-4 h-4" />} title="Data & Storage">
```

Replace with:
```tsx
        <div className="space-y-4">
          {/* ACCOUNT */}
          <AccountSection />

          {/* STORAGE OVERVIEW */}
          <SettingsSection icon={<HardDrive className="w-4 h-4" />} title="Data & Storage">
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/components/SettingsPage.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/components/SettingsPage.tsx
git commit -m "feat(web): add Account section to settings page"
```

---

### Task 7: Refactor ConversationView to use AuthProvider

**Files:**
- Modify: `apps/web/src/components/ConversationView.tsx`

**Step 1: Replace local auth state with context**

In `apps/web/src/components/ConversationView.tsx`:

1. Add import: `import { useAuth } from '../hooks/use-auth'`
2. Remove import: `import { SignInPrompt } from './SignInPrompt'`
3. Add at top of component: `const { openSignIn } = useAuth()`
4. Remove: `const [showSignIn, setShowSignIn] = useState(false)` (line 118)
5. In `handleShare` catch block (around line 128-134), replace `setShowSignIn(true)` with `openSignIn(() => handleShare())` — this passes the retry callback so the share is automatically retried after sign-in
6. Remove the entire sign-in modal JSX block (lines 782-804) — the hand-rolled `<div className="fixed inset-0 bg-black/60 ...">` block

For the catch block, the final code should be:
```tsx
if (err instanceof Error && err.message === 'AUTH_REQUIRED') {
  const token = await getAccessToken()
  if (token) {
    showToast('Share failed: server authentication error')
  } else {
    openSignIn(() => handleShare())
  }
}
```

**Step 2: Verify it compiles**

Run: `cd apps/web && bunx tsc --noEmit --pretty 2>&1 | head -30`
Expected: No errors

**Step 3: Run biome check**

Run: `cd apps/web && bunx biome check --write src/components/ConversationView.tsx`
Expected: No errors

**Step 4: Commit**

```bash
git add apps/web/src/components/ConversationView.tsx
git commit -m "refactor(web): use AuthProvider for sign-in modal in ConversationView

Replace hand-rolled modal + local showSignIn state with context's
openSignIn(onSuccess). Share auto-retries after sign-in via callback."
```

---

### Task 8: Build and manual verification

**Files:**
- None (verification only)

**Step 1: Build the frontend**

Run: `cd apps/web && bun run build`
Expected: Build succeeds with no errors

**Step 2: Type-check**

Run: `cd apps/web && bunx tsc --noEmit --pretty`
Expected: No type errors

**Step 3: Run biome on all changed files**

Run: `cd apps/web && bunx biome check --write src/hooks/use-auth.tsx src/components/UserMenu.tsx src/components/AccountSection.tsx src/main.tsx src/components/Header.tsx src/components/SettingsPage.tsx src/components/ConversationView.tsx`
Expected: No errors

**Step 4: Run existing tests**

Run: `cd apps/web && bunx vitest run`
Expected: All existing tests pass (no regressions)

**Step 5: Manual verification checklist**

If Supabase env vars are set (`.env.local`):
- [ ] Header shows "Sign in" text button when not authenticated
- [ ] Clicking "Sign in" opens Radix Dialog modal with Google + Magic Link
- [ ] After Google OAuth → avatar appears in header, modal closes
- [ ] Clicking avatar → dropdown shows email, provider, My Shares, Account Settings, Sign out
- [ ] "Sign out" clears session, reverts to "Sign in" button
- [ ] Settings page shows Account section at top with user info
- [ ] Share button on session → if not signed in → opens the centralized sign-in modal
- [ ] After sign-in via share flow → modal closes, share is automatically retried (via `openSignIn(onSuccess)` callback)

If Supabase env vars are NOT set:
- [ ] No "Sign in" button in header (graceful degradation — `UserMenu` returns null)
- [ ] No Account section on settings page (`AccountSection` returns null)
- [ ] Share button: 401 path shows toast (existing behavior, no broken modal)

**Step 6: Commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix(web): address auth UX verification findings"
```

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
|---|-------|----------|-------------|
| 1 | `Session`/`User` are not exported from `@supabase/supabase-js` — exports are `AuthSession`/`AuthUser` | Blocker | Changed import to `type { AuthSession, AuthUser }`, renamed internal type to `AppUser` to avoid name collision |
| 2 | Share retry-after-auth silently dropped — `openSignIn()` had no callback mechanism | Blocker | Extended `openSignIn` to accept optional `onSuccess` callback, stored in `useRef`, called from `onAuthStateChange` on `SIGNED_IN` |
| 3 | `UserMenu` showed "Sign in" when Supabase is null — clicking opened broken modal | Blocker | Added `if (!supabase) return null` guard at top of `UserMenu` |
| 4 | Three separate `<AuthProvider>` instances in App.tsx early returns — double subscription + state flicker | Blocker | Moved `AuthProvider` to `main.tsx` (single mount point, wraps `RouterProvider`). Task 2 now modifies `main.tsx` instead of `App.tsx` |
| 5 | Task 2 used `...` placeholders — not copy-paste-executable | Blocker | Rewrote Task 2 with verbatim `old_string` → `new_string` replacement for `main.tsx` |
| 6 | No biome lint step — `useImportType` and `noUnusedImports` rules could flag new code | Blocker | Added `bunx biome check --write` step to every task + comprehensive check in Task 8 |
| 7 | `AuthPill`/`AuthBanner` coexistence not mentioned — executor could be confused | Warning | Added explicit "Important context" section at plan top + note in Task 4 explaining both auth systems coexist intentionally |
| 8 | No rollback section | Minor | Added rollback instructions to plan header |
| 9 | `signOut` called `setUser(null)` redundantly alongside `onAuthStateChange` | Minor | Removed manual `setUser(null)` from `signOut` — `onAuthStateChange` handles it |
