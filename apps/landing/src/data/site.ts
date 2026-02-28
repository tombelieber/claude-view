/**
 * Centralized constants for the landing site.
 *
 * Single source of truth for values that appear in multiple places.
 * Import from here instead of hardcoding — keeps everything in sync.
 */

// ---------------------------------------------------------------------------
// Site metadata
// ---------------------------------------------------------------------------

export const SITE_URL = 'https://claude-view.dev'
export const GITHUB_REPO = 'tombelieber/claude-view'
export const GITHUB_URL = `https://github.com/${GITHUB_REPO}`

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
}

export const PRICING = {
  free: {
    name: 'Free',
    price: '$0',
    period: 'forever',
    description: 'Everything you need for solo development.',
    features: [
      'Unlimited local sessions',
      'Session browser & search',
      'Cost tracking',
      'AI Fluency Score',
      'Community support',
    ],
    cta: 'Get Started',
    ctaHref: '/docs/',
  },
  pro: {
    name: 'Pro',
    price: '$19',
    period: '/mo',
    description: 'For power users who need cloud access.',
    features: [
      'Everything in Free',
      'Cloud relay access',
      'Mobile app',
      'Remote agent control',
      'Priority support',
    ],
    cta: 'Coming Soon',
    ctaHref: '',
    highlight: true,
    comingSoon: true,
  },
  team: {
    name: 'Team',
    price: '$49',
    period: '/mo',
    description: 'Shared dashboards for engineering teams.',
    features: [
      'Everything in Pro',
      'Team dashboard',
      'Shared session history',
      'Usage analytics',
      'SSO & admin controls',
    ],
    cta: 'Coming Soon',
    ctaHref: '',
    comingSoon: true,
  },
} as const satisfies Record<string, PricingTier>

/** Ordered array for rendering pricing cards. */
export const PRICING_TIERS: PricingTier[] = [PRICING.free, PRICING.pro, PRICING.team]

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
// MCP
// ---------------------------------------------------------------------------

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
export const MCP_PACKAGE = '@claude-view/mcp'

// ---------------------------------------------------------------------------
// Deep links
// ---------------------------------------------------------------------------

export const DEEP_LINK_SCHEME = 'claude-view'

// ---------------------------------------------------------------------------
// Default port
// ---------------------------------------------------------------------------

export const DEFAULT_PORT = 47892
