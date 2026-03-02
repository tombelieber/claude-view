import { useEffect, useState } from 'react'
import { supabase } from '../lib/supabase'

interface Props {
  onSignedIn: () => void
}

export function SignInPrompt({ onSignedIn }: Props) {
  const [email, setEmail] = useState('')
  const [sent, setSent] = useState(false)
  const [loading, setLoading] = useState(false)

  // Listen for auth state changes (e.g. after OAuth redirect or magic link)
  useEffect(() => {
    if (!supabase) return
    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((event) => {
      if (event === 'SIGNED_IN') onSignedIn()
    })
    return () => subscription.unsubscribe()
  }, [onSignedIn])

  const handleMagicLink = async () => {
    if (!supabase) return
    if (!email.trim()) return
    setLoading(true)
    const { error } = await supabase.auth.signInWithOtp({
      email: email.trim(),
      options: { emailRedirectTo: window.location.href },
    })
    setLoading(false)
    if (!error) setSent(true)
  }

  const handleGoogle = async () => {
    if (!supabase) return
    await supabase.auth.signInWithOAuth({
      provider: 'google',
      options: { redirectTo: window.location.href },
    })
  }

  if (sent) {
    return (
      <div className="text-center py-8">
        <p className="text-gray-700 dark:text-gray-300">Check your email for the sign-in link.</p>
        <p className="text-gray-500 dark:text-gray-400 text-sm mt-2">
          You can close this and use the link from any device.
        </p>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-3 p-6 max-w-sm mx-auto">
      <h2 className="text-gray-800 dark:text-gray-200 font-medium">Sign in to enable sharing</h2>
      <p className="text-gray-500 dark:text-gray-400 text-sm">
        One account for sharing and mobile sync.
      </p>

      <button
        type="button"
        onClick={handleGoogle}
        className="flex items-center justify-center gap-2 px-4 py-2 rounded-md
          bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100
          border border-gray-300 dark:border-gray-600
          hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors text-sm font-medium"
      >
        Continue with Google
      </button>

      <div className="flex items-center gap-2 text-gray-400 dark:text-gray-600 text-xs">
        <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
        or
        <div className="flex-1 border-t border-gray-200 dark:border-gray-700" />
      </div>

      <input
        type="email"
        placeholder="your@email.com"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && handleMagicLink()}
        className="px-3 py-2 rounded-md bg-white dark:bg-gray-900
          border border-gray-300 dark:border-gray-700
          text-gray-800 dark:text-gray-200 placeholder-gray-400 dark:placeholder-gray-600
          text-sm focus:outline-none focus:border-blue-500 dark:focus:border-blue-400"
      />
      <button
        type="button"
        onClick={handleMagicLink}
        disabled={loading || !email.trim()}
        className="px-4 py-2 rounded-md bg-blue-600 hover:bg-blue-500
          text-white text-sm font-medium transition-colors disabled:opacity-50"
      >
        {loading ? 'Sending...' : 'Send magic link'}
      </button>
    </div>
  )
}
