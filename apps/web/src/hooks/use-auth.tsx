import * as Dialog from '@radix-ui/react-dialog'
import type { AuthSession, AuthUser } from '@supabase/supabase-js'
import { createContext, useCallback, useContext, useEffect, useRef, useState } from 'react'
import { SignInPrompt } from '../components/SignInPrompt'
import { DialogContent, DialogOverlay } from '../components/ui/CenteredDialog'
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
    supabase.auth
      .getSession()
      .then(({ data: { session } }) => {
        if (session?.user) {
          setUser(mapUser(session.user, session))
        }
      })
      .catch((err) => {
        console.error('[auth] getSession failed:', err)
      })
      .finally(() => {
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
      <Dialog.Root
        open={signInOpen}
        onOpenChange={(open) => {
          setSignInOpen(open)
          if (!open) onSignInSuccessRef.current = undefined
        }}
      >
        <Dialog.Portal>
          <DialogOverlay className="bg-black/60 data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0" />
          <DialogContent className="bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-xl shadow-2xl">
            <Dialog.Title className="sr-only">Sign in</Dialog.Title>
            <Dialog.Description className="sr-only">
              Sign in with Google or email to enable sharing and sync
            </Dialog.Description>
            <SignInPrompt onSignedIn={() => setSignInOpen(false)} />
          </DialogContent>
        </Dialog.Portal>
      </Dialog.Root>
    </AuthContext.Provider>
  )
}

export function useAuth(): AuthContextValue {
  return useContext(AuthContext)
}
