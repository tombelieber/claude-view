/**
 * Spinner UX constants and utility functions.
 *
 * Verbs are extracted from Claude Code's own spinner vocabulary.
 * Format functions intentionally differ from format-utils.ts to match
 * the Claude Code TUI style (lowercase k, compact duration, etc.).
 */

/** 155 present-participle verbs used by Claude Code spinners */
export const SPINNER_VERBS = [
  'Appraising', 'Assessing', 'Baking', 'Brainstorming', 'Brewing',
  'Brooding', 'Calibrating', 'Charting', 'Chewing', 'Churning',
  'Coalescing', 'Cobbling', 'Cogitating', 'Communing', 'Compiling',
  'Composing', 'Computing', 'Conjuring', 'Connecting', 'Considering',
  'Contemplating', 'Convoking', 'Cooking', 'Crafting', 'Crunching',
  'Curating', 'Daydreaming', 'Deciphering', 'Deliberating', 'Delving',
  'Designing', 'Devising', 'Digesting', 'Discerning', 'Distilling',
  'Dreaming', 'Effecting', 'Elaborating', 'Elucidating', 'Envisioning',
  'Evaluating', 'Examining', 'Excavating', 'Experimenting', 'Exploring',
  'Extracting', 'Fabricating', 'Fashioning', 'Fermenting', 'Figuring',
  'Forging', 'Formulating', 'Fusing', 'Generating', 'Germinating',
  'Grappling', 'Grinding', 'Harmonizing', 'Hatching', 'Hypothesizing',
  'Ideating', 'Illuminating', 'Imagining', 'Improvising', 'Incubating',
  'Infusing', 'Innovating', 'Inspecting', 'Integrating', 'Investigating',
  'Iterating', 'Juggling', 'Kindling', 'Kneading', 'Machinating',
  'Mapping', 'Marinating', 'Meandering', 'Meditating', 'Melding',
  'Milling', 'Mixing', 'Mulling', 'Musing', 'Navigating',
  'Noodling', 'Orchestrating', 'Parsing', 'Percolating', 'Pickling',
  'Piecing', 'Planning', 'Plodding', 'Pondering', 'Preparing',
  'Processing', 'Probing', 'Puzzling', 'Questioning', 'Racking',
  'Reasoning', 'Reconciling', 'Refining', 'Reflecting', 'Researching',
  'Resolving', 'Ruminating', 'Sauteing', 'Scanning', 'Scheming',
  'Searching', 'Shaping', 'Sifting', 'Simulating', 'Sketching',
  'Smelting', 'Solving', 'Sorting', 'Sparking', 'Speculating',
  'Spinning', 'Steeping', 'Stewing', 'Stitching', 'Strategizing',
  'Streamlining', 'Structuring', 'Studying', 'Surveying', 'Synthesizing',
  'Tending', 'Thinking', 'Tinkering', 'Toiling', 'Tracing',
  'Tuning', 'Turning', 'Tweaking', 'Twiddling', 'Unfolding',
  'Unpacking', 'Unraveling', 'Untangling', 'Unweaving', 'Venturing',
  'Wandering', 'Weaving', 'Weighing', 'Whittling', 'Winding',
  'Wondering', 'Working', 'Wrestling', 'Wrangling', 'Zeroing',
] as const

/** Past-tense verbs shown briefly when a session completes */
export const PAST_TENSE_VERBS = [
  'Baked', 'Brewed', 'Churned', 'Cogitated',
  'Cooked', 'Crunched', 'Sauteed', 'Worked',
] as const

/** Unicode frames for the spinning animation */
export const SPINNER_FRAMES = ['·', '✢', '✳', '✶', '✻', '✽'] as const

/** djb2-style hash, returns a non-negative integer */
function hashSessionId(sessionId: string): number {
  let hash = 0
  for (let i = 0; i < sessionId.length; i++) {
    hash = ((hash << 5) - hash + sessionId.charCodeAt(i)) | 0
  }
  return Math.abs(hash)
}

/**
 * Deterministic verb selection based on session ID.
 * Same session always gets the same verb.
 */
export function pickVerb(sessionId: string): string {
  return SPINNER_VERBS[hashSessionId(sessionId) % SPINNER_VERBS.length]
}

/**
 * Deterministic past-tense verb selection based on session ID.
 * Same hash, indexes into PAST_TENSE_VERBS.
 */
export function pickPastVerb(sessionId: string): string {
  return PAST_TENSE_VERBS[hashSessionId(sessionId) % PAST_TENSE_VERBS.length]
}

/**
 * Format token counts in compact style: "18.3k", "1.2M".
 *
 * Uses lowercase k to match Claude Code TUI convention.
 * Intentionally different from formatNumber() in format-utils.ts
 * which uses uppercase K and locale formatting.
 */
export function formatTokensCompact(n: number): string {
  if (!Number.isFinite(n) || n < 0) return '0'
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}k`
  return `${n}`
}

/**
 * Format model name in short form: "claude-opus-4-6" -> "opus-4.6".
 *
 * Handles both naming conventions:
 *   claude-{family}-{major}-{minor}-{date}  (e.g. claude-opus-4-6-20260210)
 *   claude-{major}-{minor}-{family}-{date}  (e.g. claude-3-5-sonnet-20240620)
 *
 * Returns null for null input. Returns original string if no pattern matches.
 */
export function formatModelShort(model: string | null): string | null {
  if (!model) return null

  // Pattern: claude-{family}-{major}[-{minor}][-{date}]
  const m = model.match(/^claude-([a-z]+)-(\d+)(?:-(\d{1,2}))?(?:-\d{8})?$/)
  if (m) {
    const [, family, major, minor] = m
    return minor ? `${family}-${major}.${minor}` : `${family}-${major}`
  }

  // Pattern: claude-{major}[-{minor}]-{family}[-{date}]
  const l = model.match(/^claude-(\d+)(?:-(\d{1,2}))?-([a-z]+)(?:-\d{8})?$/)
  if (l) {
    const [, major, minor, family] = l
    return minor ? `${family}-${major}.${minor}` : `${family}-${major}`
  }

  return model
}

/**
 * Format duration in compact Claude Code TUI style.
 *
 * - < 60s: "45s"
 * - < 3600s: "12m"
 * - >= 3600s: "2h 15m"
 * - 0 or negative: "0s"
 */
export function formatDurationCompact(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0s'
  if (seconds < 60) return `${Math.floor(seconds)}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return m > 0 ? `${h}h ${m}m` : `${h}h`
}
