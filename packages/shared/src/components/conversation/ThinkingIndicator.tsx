import { useEffect, useRef, useState } from 'react'

// ── Verb sets by phase ────────────────────────────────────────────────────────

const VERB_SETS = {
  loading: ['Loading conversation', 'Fetching messages', 'Preparing view'],
  connecting: ['Connecting', 'Setting up session', 'Initializing', 'Preparing'],
  thinking: ['Thinking', 'Processing', 'Analyzing', 'Reasoning', 'Composing'],
} as const

/** Past-tense labels shown briefly when a phase completes. */
const COMPLETED_LABELS: Record<ThinkingPhase, string> = {
  loading: 'Loaded',
  connecting: 'Connected',
  thinking: 'Done',
}

export type ThinkingPhase = keyof typeof VERB_SETS

// ── Keyframes (injected once) ─────────────────────────────────────────────────

const STYLE_ID = 'thinking-indicator-keyframes'

function ensureKeyframes() {
  if (document.getElementById(STYLE_ID)) return
  const style = document.createElement('style')
  style.id = STYLE_ID
  style.textContent = `
    @keyframes thinking-dot-bounce {
      0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
      30% { transform: translateY(-4px); opacity: 1; }
    }
    @keyframes thinking-verb-in {
      from { opacity: 0; transform: translateY(4px); }
      to { opacity: 1; transform: translateY(0); }
    }
    @keyframes thinking-phase-done {
      0%   { opacity: 0; transform: translateY(4px); }
      12%  { opacity: 1; transform: translateY(0); }
      75%  { opacity: 1; }
      100% { opacity: 0; }
    }
  `
  document.head.appendChild(style)
}

// ── Component ─────────────────────────────────────────────────────────────────

interface Props {
  phase: ThinkingPhase
  /** Center in full available space (used when no messages exist yet). */
  centered?: boolean
}

export function ThinkingIndicator({ phase, centered }: Props) {
  const verbs = VERB_SETS[phase] ?? VERB_SETS.loading
  const [index, setIndex] = useState(0)
  const injectedRef = useRef(false)

  // ── Lifecycle trail: track completed phase ──────────────────────────────
  const [completedPhase, setCompletedPhase] = useState<ThinkingPhase | null>(null)
  const prevPhaseRef = useRef(phase)

  useEffect(() => {
    if (prevPhaseRef.current !== phase) {
      const prev = prevPhaseRef.current
      prevPhaseRef.current = phase
      setCompletedPhase(prev)
      // Auto-clear after the fade-out animation finishes
      const timer = setTimeout(() => setCompletedPhase(null), 2500)
      return () => clearTimeout(timer)
    }
  }, [phase])

  // Inject keyframes once
  useEffect(() => {
    if (!injectedRef.current) {
      ensureKeyframes()
      injectedRef.current = true
    }
  }, [])

  // Reset verb index on phase change
  useEffect(() => {
    setIndex(0)
  }, [phase])

  // Cycle verbs
  useEffect(() => {
    const timer = setInterval(() => {
      setIndex((i) => (i + 1) % verbs.length)
    }, 4000)
    return () => clearInterval(timer)
  }, [verbs])

  // ── Rendered pieces ────────────────────────────────────────────────────

  const dots = (
    <div className="flex items-center gap-[3px]">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className="w-[5px] h-[5px] rounded-full bg-blue-400 dark:bg-blue-500"
          style={{
            animation: 'thinking-dot-bounce 1.4s infinite ease-in-out',
            animationDelay: `${i * 0.16}s`,
          }}
        />
      ))}
    </div>
  )

  const content = (
    <div className="flex flex-col gap-1">
      {/* Completed phase — fades in then out over 2.5s */}
      {completedPhase && (
        <div
          key={`done-${completedPhase}`}
          className="flex items-center gap-1.5"
          style={{ animation: 'thinking-phase-done 2.5s ease-out forwards' }}
        >
          <svg
            className="w-3.5 h-3.5 text-green-400 dark:text-green-500"
            viewBox="0 0 16 16"
            fill="none"
          >
            <title>Done</title>
            <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
            <path
              d="M5 8l2 2 4-4"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <span className="text-xs text-green-400/80 dark:text-green-500/70">
            {COMPLETED_LABELS[completedPhase]}
          </span>
        </div>
      )}
      {/* Active phase — bouncing dots + cycling verb */}
      <div className="flex items-center gap-2.5">
        {dots}
        <span
          key={`${phase}-${index}`}
          className="text-sm text-gray-400 dark:text-gray-500"
          style={{ animation: 'thinking-verb-in 0.3s ease-out' }}
        >
          {verbs[index]}
        </span>
      </div>
    </div>
  )

  if (centered) {
    return <div className="flex items-center justify-center flex-1">{content}</div>
  }

  return <div className="max-w-3xl mx-auto px-4 py-3">{content}</div>
}
