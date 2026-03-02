# Web App Auth UX Design

**Date:** 2026-03-02
**Status:** Done (implemented 2026-03-02)
**Scope:** `apps/web/` only (no mobile, no landing)

## Problem

Auth plumbing exists (Supabase client, JWT validation, SignInPrompt modal) but there is zero user-facing auth UX. Users who sign in to share have no way to see who they're logged in as, no logout button, and no persistent auth indicator. More auth-gated features are coming (cloud sync, preferences, session history), so a proper auth foundation is needed now.

## Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| Approach | Auth Pill + Settings section | Proper presence + foundation for future features |
| Entry point | Header avatar/button (top-right) | Standard SaaS pattern (GitHub, Linear, Vercel) |
| Sign-in CTA | Always visible in header | Users can sign in proactively |
| Sign-in flow | Modal/dialog (Radix) | Non-disruptive, user stays in context |
| State management | React context (`AuthProvider`) | Global user state, any component can trigger sign-in |

## Architecture

### AuthProvider Context

Wraps the entire app. Manages Supabase auth state globally.

```
AuthProvider (wraps <App />)
  ├── state: { user: User | null, loading: boolean }
  ├── signOut(): void
  ├── openSignIn(): void  ← controls the modal
  └── listens to supabase.auth.onAuthStateChange()
```

**Behavior:**
- On mount: `supabase.auth.getSession()` to restore existing session
- Subscribes to `onAuthStateChange` for login/logout/token refresh
- User object: `{ id, email, displayName, avatarUrl, provider }`
- When `supabase` is `null` (no env vars): `{ user: null, loading: false }` — graceful degradation
- Sign-in modal is **owned by the provider**, so any feature can call `openSignIn()`

Replaces the current pattern where `ConversationView` manages its own `showSignIn` state.

### UserMenu Component (Header)

**Signed out:**
```
[... existing header items ...] [Sign in]  [⚙]
```
- Small text button, subtle — not a big CTA
- Clicking calls `openSignIn()` from context

**Signed in:**
```
[... existing header items ...] [TB ▾]  [⚙]
```
- 32px circular avatar:
  1. Google avatar (if OAuth)
  2. Gravatar (if email has one)
  3. Initials fallback (colored bg)
- Radix UI Popover dropdown on click

**Dropdown contents:**
```
┌──────────────────────────┐
│  tom@example.com         │
│  via Google               │
│──────────────────────────│
│  ○ My Shares  →          │
│  ○ Account Settings  →   │
│──────────────────────────│
│  ○ Sign out               │
└──────────────────────────┘
```

### Sign-In Modal (Refactored)

Existing `SignInPrompt` UI (Google OAuth + Magic Link) lifted into `AuthProvider`:
- Radix Dialog instead of hand-rolled backdrop
- Any component calls `openSignIn()` → modal appears
- Auto-closes on successful auth via `onAuthStateChange`
- Backdrop click or X dismisses

### Settings Page: Account Section

New `SettingsSection` at the **top** of the settings page.

**Signed in:**
```
┌──────────────────────────────────────────────┐
│ 👤  ACCOUNT                                   │
│──────────────────────────────────────────────│
│  [Avatar]  tom@example.com                   │
│            Signed in via Google               │
│            Member since Mar 2026             │
│  [Sign out]                                  │
└──────────────────────────────────────────────┘
```

**Signed out:**
```
┌──────────────────────────────────────────────┐
│ 👤  ACCOUNT                                   │
│──────────────────────────────────────────────│
│  Sign in to enable sharing and sync.         │
│  [Sign in with Google]  [Sign in with Email] │
└──────────────────────────────────────────────┘
```

## Data Flow

```
supabase.auth.onAuthStateChange()
  → AuthProvider updates user state
  → Header re-renders UserMenu (avatar or "Sign in")
  → Settings page re-renders Account section
  → Share flow uses getAccessToken() (unchanged)
```

No new API endpoints. No new database tables. Pure frontend state from Supabase SDK.

## Files Changed

| File | Change |
|------|--------|
| `hooks/use-auth.ts` | **NEW** — `AuthProvider` context + `useAuth` hook |
| `components/UserMenu.tsx` | **NEW** — header avatar + Radix Popover dropdown |
| `components/AccountSection.tsx` | **NEW** — settings page account section |
| `components/SignInPrompt.tsx` | **MODIFY** — wrap in Radix Dialog, remove hand-rolled backdrop |
| `components/Header.tsx` | **MODIFY** — add `UserMenu` to right side |
| `components/SettingsPage.tsx` | **MODIFY** — add `AccountSection` at top |
| `components/ConversationView.tsx` | **MODIFY** — remove local `showSignIn` state, use `openSignIn()` from context |
| `App.tsx` | **MODIFY** — wrap with `AuthProvider` |

## Design Principles

- **Graceful degradation:** App works fully without auth. No env vars = no auth UI shown.
- **Non-disruptive:** Modal sign-in keeps user in context.
- **Standard patterns:** Radix UI for overlays, avatar+dropdown is universal SaaS UX.
- **Single source of truth:** `AuthProvider` owns all auth state. No duplicate auth logic in individual components.

## Not In Scope

- Dedicated `/login` or `/profile` routes
- Avatar upload or display name editing
- Auth-gated route guards (individual features gate themselves)
- Mobile app auth (separate design)
- Password auth (Supabase OAuth + Magic Link only)
