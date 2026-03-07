/**
 * Centralized constants for the landing site.
 *
 * Single source of truth for values that appear in multiple places.
 * Import from here instead of hardcoding — keeps everything in sync.
 */

// ---------------------------------------------------------------------------
// Site metadata
// ---------------------------------------------------------------------------

export const SITE_URL = 'https://claudeview.ai'
export const GITHUB_REPO = 'tombelieber/claude-view'
export const GITHUB_URL = `https://github.com/${GITHUB_REPO}`

/** Must match the version in root package.json and Cargo.toml workspace. */
export const VERSION = '0.9.0'

/** Set to null to disable Twitter card meta tag. Update when handle is verified. */
export const TWITTER_HANDLE: string | null = null

// ---------------------------------------------------------------------------
// Pricing
// ---------------------------------------------------------------------------

export interface PricingTier {
  name: string
  price: string
  period: string
  description: string
  features: string[]
  cta: string
  ctaHref: string
  highlight?: boolean
  comingSoon?: boolean
  /** Per-user pricing (shown as "/user/mo" instead of "/mo"). */
  perUser?: boolean
}

export const PRICING = {
  hobby: {
    name: 'Hobby',
    price: '$0',
    period: 'forever',
    description: 'See what your Claude Code is actually doing.',
    features: [
      'Limited session monitoring',
      'Session browser & search',
      'Basic cost tracking',
      'Community support',
    ],
    cta: 'Get Started',
    ctaHref: '/docs/',
  },
  pro: {
    name: 'Pro',
    price: '$20',
    period: '/mo',
    description: 'Full control of your Claude Code from anywhere.',
    features: [
      'Everything in Hobby, plus:',
      'Unlimited session monitoring',
      'Cloud relay — work from any device',
      'Mobile app (monitor + control)',
      'Remote agent control',
      'Plan runner & workflows',
      'Agent orchestration engine',
      'Quality gates (auto-review)',
      'AI Fluency Score & full analytics',
    ],
    cta: 'Join Waitlist',
    ctaHref: '',
    highlight: true,
    comingSoon: true,
  },
  proPlus: {
    name: 'Pro+',
    price: '$60',
    period: '/mo',
    description: '3x usage for heavier workloads.',
    features: ['Everything in Pro, plus:', '3x usage on all features', 'Priority support'],
    cta: 'Join Waitlist',
    ctaHref: '',
    comingSoon: true,
  },
  ultra: {
    name: 'Ultra',
    price: '$200',
    period: '/mo',
    description: 'Maximum power for all-day agent fleets.',
    features: [
      'Everything in Pro, plus:',
      '20x usage on all features',
      'Priority access to new features',
      'Priority support',
    ],
    cta: 'Join Waitlist',
    ctaHref: '',
    comingSoon: true,
  },
  business: {
    name: 'Business',
    price: '$40',
    period: '/mo',
    perUser: true,
    description: 'Your whole team, one dashboard.',
    features: [
      'Everything in Pro, plus:',
      'Team dashboard & shared sessions',
      'Usage analytics & reporting',
      'Centralized team billing',
      'Role-based access control',
      'SAML/OIDC SSO',
      'Org-wide privacy controls',
    ],
    cta: 'Join Waitlist',
    ctaHref: '',
    comingSoon: true,
  },
  enterprise: {
    name: 'Enterprise',
    price: 'Custom',
    period: '',
    description: 'Org-wide deployment with full compliance.',
    features: [
      'Everything in Business, plus:',
      'Pooled usage across org',
      'Invoice / PO billing',
      'SCIM seat management',
      'Audit logs & compliance',
      'Dedicated support & account management',
    ],
    cta: 'Contact Sales',
    ctaHref: '',
    comingSoon: true,
  },
} as const satisfies Record<string, PricingTier>

/** Individual plans — shown in the main pricing grid. */
export const INDIVIDUAL_TIERS: PricingTier[] = [
  PRICING.hobby,
  PRICING.pro,
  PRICING.proPlus,
  PRICING.ultra,
]

/** Business plans — shown below the individual grid. */
export const BUSINESS_TIERS: PricingTier[] = [PRICING.business, PRICING.enterprise]

/** All tiers in order. */
export const PRICING_TIERS: PricingTier[] = [...INDIVIDUAL_TIERS, ...BUSINESS_TIERS]

// ---------------------------------------------------------------------------
// Platform support
// ---------------------------------------------------------------------------

export const PLATFORM = {
  current: 'macOS',
  nodeMin: '18',
  linuxVersion: 'v2.1',
  windowsVersion: 'v2.2',
  macosMin: '12 (Monterey)',
} as const

// ---------------------------------------------------------------------------
// Plugin — Claude Code plugin (@claude-view/plugin)
// ---------------------------------------------------------------------------

export const PLUGIN_PACKAGE = '@claude-view/plugin'

export const MCP_TOOLS = [
  {
    name: 'list_sessions',
    description: 'Browse sessions with filters (project, model, date, status)',
  },
  { name: 'get_session', description: 'Session detail with messages, tool calls, and metrics' },
  { name: 'search_sessions', description: 'Full-text search across all conversations' },
  { name: 'get_stats', description: 'Dashboard overview — total sessions, costs, trends' },
  { name: 'get_fluency_score', description: 'AI Fluency Score (0-100) with breakdown' },
  { name: 'get_token_stats', description: 'Token usage with cache hit ratio' },
  { name: 'list_live_sessions', description: 'Currently running agents (real-time)' },
  { name: 'get_live_summary', description: 'Aggregate cost and status for today' },
] as const

export const MCP_TOOL_COUNT = MCP_TOOLS.length

export const PLUGIN_SKILLS = [
  { name: '/session-recap', description: 'Summarize a specific session' },
  { name: '/daily-cost', description: "Today's spending and activity report" },
  { name: '/standup', description: 'Multi-session work log for standups' },
] as const

export const PLUGIN_SKILL_COUNT = PLUGIN_SKILLS.length

// ---------------------------------------------------------------------------
// Deep links
// ---------------------------------------------------------------------------

export const DEEP_LINK_SCHEME = 'claude-view'

// ---------------------------------------------------------------------------
// Default port
// ---------------------------------------------------------------------------

export const DEFAULT_PORT = 47892

// ---------------------------------------------------------------------------
// Environment
// ---------------------------------------------------------------------------

/**
 * Build-time environment flag. Set PUBLIC_ENV=preview to disable SEO
 * (noindex, no structured data, no sitemap references).
 * Defaults to 'production'.
 */
export const IS_PREVIEW = import.meta.env.PUBLIC_ENV === 'preview'

// ---------------------------------------------------------------------------
// Waitlist
// ---------------------------------------------------------------------------

export const WAITLIST_API = '/api/waitlist'

/**
 * Cloudflare Turnstile site key (public, safe to embed in client code).
 * Set PUBLIC_TURNSTILE_SITE_KEY in your Astro environment to use a real key.
 * Falls back to Cloudflare's always-passes test key for local development.
 */
export const TURNSTILE_SITE_KEY =
  import.meta.env.PUBLIC_TURNSTILE_SITE_KEY ?? '1x00000000000000000000AA'
