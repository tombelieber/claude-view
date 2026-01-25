# Session Discovery UX - Design Specification

> Visual design system and component mockups for Claude View session discovery features.

---

## Design Philosophy

**Aesthetic Direction:** Editorial minimalism meets developer tooling

Think: Linear's command palette + Raycast's polish + a well-designed indie dev blog

**Core Principles:**

- **Recognition over recall** - Stats and patterns help users find sessions without remembering
- **Everything is a search entry point** - Clickable stats, skills, files all trigger filtered searches
- **Information density with hierarchy** - Show more data, but with clear visual weight
- **Developer-native feel** - Monospace where appropriate, keyboard-first, no hand-holding

**Target Users:** Heavy Claude Code users with 100+ sessions who need to navigate their history efficiently.

---

## Color System

```css
:root {
  /* Base palette - warm neutrals, not clinical */
  --bg-primary: #ffffff;
  --bg-secondary: #f8f8f9;
  --bg-tertiary: #f0f0f2;

  /* Dark mode palette (command palette) */
  --bg-modal: #111113;
  --bg-modal-elevated: #1c1c1f;
  --border-modal: #2a2a2e;

  /* Text hierarchy */
  --text-primary: #1a1a1b;
  --text-secondary: #6b6b70;
  --text-muted: #9b9ba0;
  --text-inverse: #ececef;
  --text-inverse-muted: #6e6e76;

  /* Accent - sage green (calming, developer-friendly) */
  --accent-sage: #7c9885;
  --accent-sage-light: #a8c4b0;
  --accent-sage-dark: #5a7362;

  /* Semantic */
  --color-active: #22c55e;
  --color-active-muted: #86efac;
  --color-interactive: #3b82f6;
  --color-interactive-hover: #2563eb;

  /* Borders */
  --border-subtle: #e5e5e7;
  --border-default: #d4d4d8;
}
```

---

## Typography

```css
:root {
  /* Display - for headings and emphasis */
  --font-display: 'JetBrains Mono', 'SF Mono', monospace;

  /* Body - for readable content */
  --font-body: 'SF Pro Text', -apple-system, BlinkMacSystemFont, sans-serif;

  /* Code - for paths, commands, skills */
  --font-mono: 'JetBrains Mono', 'SF Mono', 'Fira Code', monospace;

  /* Scale */
  --text-xs: 0.6875rem;   /* 11px */
  --text-sm: 0.8125rem;   /* 13px */
  --text-base: 0.875rem;  /* 14px */
  --text-lg: 1rem;        /* 16px */
  --text-xl: 1.25rem;     /* 20px */
}
```

---

## Component Designs

### 1. Enhanced Session Card

**Purpose:** Give users enough context to identify a session without opening it.

**Visual Hierarchy:**
1. Started message (what they asked)
2. Ended message (where they left off)
3. Files touched (what changed)
4. Activity badges (how much happened)
5. Timestamp (when)

```
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                         │
│  Started: "fix the login bug in the auth flow"                          │
│  Ended: "looks good, let's ship it"                                     │
│                                                                         │
│  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄  │
│                                                                         │
│  📁 auth.ts, Login.tsx, api/session.ts                                  │
│                                                                         │
│  ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄  │
│                                                                         │
│  ✏️ 12    🖥️ 3    👁️ 8        /commit  /brainstorm       Friday, 12:28 AM │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘

Legend:
  ✏️ = edits     🖥️ = bash commands     👁️ = file reads
  Badges = skills/slash commands used
```

**States:**

```
┌─ DEFAULT ────────────────────────────────────────────────────────────────┐
│  bg: white                                                               │
│  border: var(--border-subtle)                                            │
│  shadow: none                                                            │
└──────────────────────────────────────────────────────────────────────────┘

┌─ HOVER ──────────────────────────────────────────────────────────────────┐
│  bg: var(--bg-secondary)                                                 │
│  border: var(--border-default)                                           │
│  shadow: 0 1px 3px rgba(0,0,0,0.04)                                      │
│  transition: all 150ms ease                                              │
└──────────────────────────────────────────────────────────────────────────┘

┌─ SELECTED ───────────────────────────────────────────────────────────────┐
│  bg: #eff6ff (blue-50)                                                   │
│  border: var(--color-interactive)                                        │
│  shadow: 0 0 0 1px var(--color-interactive)                              │
└──────────────────────────────────────────────────────────────────────────┘

┌─ ACTIVE (live session) ──────────────────────────────────────────────────┐
│  Shows pulsing green dot + "Active" label                                │
│  Green accent on left border (2px)                                       │
└──────────────────────────────────────────────────────────────────────────┘
```

**Responsive Behavior:**

- < 640px: Stack tool counts and skills vertically
- Tool counts collapse to just icons if space constrained
- Files list truncates with "+N more" indicator

---

### 2. Command Palette (⌘K Search)

**Purpose:** Fast, keyboard-driven search with query syntax for power users.

**Aesthetic:** Dark modal floating over blurred backdrop. Editorial monospace typography.

```
                    ╭──────────────────────────────────────────────────────────────────╮
                    │                                                                  │
                    │   ╭──────────────────────────────────────────────────────────╮   │
                    │   │  ⌘   project:fluffy auth█                                │   │
                    │   ╰──────────────────────────────────────────────────────────╯   │
                    │                                                                  │
                    │   ──────────────────────────────────────────────────────────────  │
                    │                                                                  │
                    │   RECENT                                                         │
                    │                                                                  │
                    │   ○  project:claude-view                           2 hours ago   │
                    │   ○  path:*.tsx "component"                        yesterday     │
                    │   ○  skill:commit                                  3 days ago    │
                    │                                                                  │
                    │   ──────────────────────────────────────────────────────────────  │
                    │                                                                  │
                    │   FILTERS                                                        │
                    │                                                                  │
                    │   ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │
                    │   │ project: │ │  path:   │ │  skill:  │ │  after:  │           │
                    │   └──────────┘ └──────────┘ └──────────┘ └──────────┘           │
                    │                                                                  │
                    │   ┌──────────┐ ┌──────────┐                                      │
                    │   │ "phrase" │ │ /regex/  │                                      │
                    │   └──────────┘ └──────────┘                                      │
                    │                                                                  │
                    │   ──────────────────────────────────────────────────────────────  │
                    │                                                                  │
                    │   ↑↓ Navigate     ⏎ Search     ⎋ Close                           │
                    │                                                                  │
                    ╰──────────────────────────────────────────────────────────────────╯
```

**Color Specification:**

```css
.command-palette {
  background: var(--bg-modal);           /* #111113 */
  border: 1px solid var(--border-modal); /* #2a2a2e */
  border-radius: 12px;
  box-shadow:
    0 25px 50px -12px rgba(0, 0, 0, 0.5),
    0 0 0 1px rgba(255, 255, 255, 0.05);
}

.command-palette__input {
  background: var(--bg-modal-elevated);  /* #1c1c1f */
  color: var(--text-inverse);            /* #ececef */
  font-family: var(--font-mono);
  font-size: var(--text-sm);
}

.command-palette__input::placeholder {
  color: var(--text-inverse-muted);      /* #6e6e76 */
}

.command-palette__filter-chip {
  background: var(--bg-modal-elevated);
  color: var(--accent-sage);             /* #7c9885 */
  border: 1px solid var(--border-modal);
  font-family: var(--font-mono);
  font-size: var(--text-xs);
}

.command-palette__filter-chip:hover {
  background: #252525;
  color: var(--accent-sage-light);
}
```

**Interaction Flow:**

1. User presses ⌘K → Modal fades in (150ms ease-out)
2. Input auto-focused, cursor blinking
3. Typing highlights recognized keywords in sage green
4. Clicking filter chip inserts it at cursor position
5. Recent searches clickable to populate input
6. Enter executes search, closes modal
7. Escape closes without searching
8. Click outside closes without searching

**Keyboard Navigation:**

| Key | Action |
|-----|--------|
| ⌘K | Open palette |
| ⎋ (Escape) | Close palette |
| ⏎ (Enter) | Execute search |
| ↑↓ | Navigate recent searches |
| Tab | Cycle through filter chips |

---

### 3. Stats Dashboard (Global)

**Purpose:** Show usage patterns to help users discover what to search for.

**Layout:** Card-based, fits in main content area when no project selected.

```
╭─────────────────────────────────────────────────────────────────────────────────╮
│                                                                                 │
│  📊  YOUR CLAUDE CODE USAGE                                                     │
│  ═══════════════════════════════════════════════════════════════════════════    │
│                                                                                 │
│  483 sessions  ·  8 projects  ·  since Dec 2025                                 │
│                                                                                 │
│  ───────────────────────────────────────────────────────────────────────────    │
│                                                                                 │
│  ⚡ TOP SKILLS                                                                   │
│                                                                                 │
│  /superpowers:brainstorm        ████████████████████░░░░░░  47                  │
│  /commit                        ████████████░░░░░░░░░░░░░░  32                  │
│  /review-pr                     ██████░░░░░░░░░░░░░░░░░░░░  18                  │
│  /superpowers:writing-plans     ████░░░░░░░░░░░░░░░░░░░░░░  12                  │
│  /debug                         ███░░░░░░░░░░░░░░░░░░░░░░░   9                  │
│                                                                                 │
│  ───────────────────────────────────────────────────────────────────────────    │
│                                                                                 │
│  📁 MOST ACTIVE PROJECTS                                                        │
│                                                                                 │
│  claude-view         ●1 active     54 sessions   ████████████░░░░░░             │
│  fluffy              ○             301 sessions  ██████████████████             │
│  web                 ○             71 sessions   █████░░░░░░░░░░░░░             │
│  taipofire-donations ○             44 sessions   ███░░░░░░░░░░░░░░░             │
│  @vicky-ai           ○             13 sessions   █░░░░░░░░░░░░░░░░░             │
│                                                                                 │
│  ───────────────────────────────────────────────────────────────────────────    │
│                                                                                 │
│  📅 ACTIVITY HEATMAP (last 30 days)                                             │
│                                                                                 │
│       W1      W2      W3      W4      W5                                        │
│  Mon  ░░▓▓░░▓▓▓▓░░░░▓▓██▓▓▓▓░░░░▓▓░░                                            │
│  Tue  ░░░░▓▓░░▓▓░░▓▓▓▓▓▓████░░▓▓░░░░                                            │
│  Wed  ▓▓░░░░▓▓▓▓░░░░▓▓██████▓▓▓▓░░░░                                            │
│  Thu  ░░▓▓▓▓░░░░▓▓▓▓░░▓▓████▓▓░░▓▓░░                                            │
│  Fri  ▓▓░░▓▓▓▓░░▓▓░░▓▓████████▓▓▓▓░░                                            │
│  Sat  ░░░░░░░░▓▓░░░░░░░░▓▓░░░░░░░░░░                                            │
│  Sun  ░░░░░░░░░░░░░░░░▓▓░░░░░░░░░░░░                                            │
│                                                                                 │
│       ░ = 0    ▓ = 1-3    █ = 4+  sessions                                      │
│                                                                                 │
╰─────────────────────────────────────────────────────────────────────────────────╯
```

**Interactive Elements:**

| Element | Hover | Click |
|---------|-------|-------|
| Skill bar | Highlight in blue | Search: `skill:brainstorm` |
| Project row | Highlight row | Select project in sidebar |
| Heatmap cell | Show tooltip "Jan 20: 4 sessions" | Search: `after:2026-01-20 before:2026-01-21` |

**Bar Chart Styling:**

```css
.stats-bar {
  height: 6px;
  background: var(--bg-tertiary);
  border-radius: 3px;
  overflow: hidden;
}

.stats-bar__fill {
  height: 100%;
  background: var(--accent-sage);
  border-radius: 3px;
  transition: width 300ms ease-out, background 150ms ease;
}

.stats-bar:hover .stats-bar__fill {
  background: var(--color-interactive);
}

/* Active project gets green bar */
.stats-bar--active .stats-bar__fill {
  background: var(--color-active);
}
```

---

### 4. Per-Project Stats (Sidebar)

**Purpose:** When a project is selected, show that project's patterns below the project list.

**Layout:** Compact vertical stack in sidebar footer.

```
╭─────────────────────────────────╮
│  claude-view                    │  ← Selected project header
│  /Users/TBGor/dev/@vicky-ai/... │
│                                 │
│  ●1 active · 54 sessions        │
│                                 │
│  ─────────────────────────────  │
│                                 │
│  SKILLS                         │
│  ┌────────────────┐ ┌─────────┐ │
│  │ /brainstorm 12 │ │ /commit │ │
│  └────────────────┘ │    8    │ │
│  ┌────────────────┐ └─────────┘ │
│  │ /review-pr  3  │             │
│  └────────────────┘             │
│                                 │
│  ─────────────────────────────  │
│                                 │
│  TOP FILES                      │
│  sessions.ts              9     │
│  App.tsx                  7     │
│  SessionCard.tsx          5     │
│                                 │
│  ─────────────────────────────  │
│                                 │
│  TOOLS                          │
│  Edit     ████████░░░░   142    │
│  Read     ██████░░░░░░    89    │
│  Bash     ████░░░░░░░░    54    │
│                                 │
╰─────────────────────────────────╯
```

**Skill Badges:**

```css
.skill-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: 4px;
  transition: all 150ms ease;
}

.skill-badge:hover {
  background: var(--color-interactive);
  color: white;
}

.skill-badge__count {
  color: var(--text-muted);
}
```

---

### 5. Search Results View

**Purpose:** Display filtered session list when search is active.

```
╭─────────────────────────────────────────────────────────────────────────────────╮
│                                                                                 │
│  SEARCH RESULTS                                           ┌─────────────────┐   │
│  12 sessions matching "project:fluffy auth"               │  Clear search   │   │
│                                                           └─────────────────┘   │
│                                                                                 │
│  ═══════════════════════════════════════════════════════════════════════════    │
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │  Started: "implement auth middleware for API routes"                     │   │
│  │  Ended: "all tests passing, ready for review"                            │   │
│  │                                                                          │   │
│  │  📁 middleware/auth.ts, routes/api.ts                                    │   │
│  │  ✏️ 8   🖥️ 2   👁️ 5     /commit                          Jan 24, 3:42 PM │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │  Started: "fix the auth token refresh bug"                               │   │
│  │  Ended: "deployed to staging"                                            │   │
│  │                                                                          │   │
│  │  📁 lib/auth.ts, hooks/useAuth.ts                                        │   │
│  │  ✏️ 4   🖥️ 1   👁️ 3     /debug                           Jan 23, 11:15 AM │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │  ...                                                                     │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
╰─────────────────────────────────────────────────────────────────────────────────╯
```

**Highlight Matching Terms:**

```css
.search-highlight {
  background: rgba(124, 152, 133, 0.2);  /* sage with transparency */
  color: var(--accent-sage-dark);
  padding: 0 2px;
  border-radius: 2px;
}
```

---

### 6. Header Search Button

**Purpose:** Visible entry point for search alongside ⌘K shortcut.

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                                                                                 │
│  Claude View                                   ┌─────────────────────────────┐  │
│                                                │  🔍  Search          ⌘K     │  │
│                                                └─────────────────────────────┘  │
│                                                                        ❓  ⚙️   │
└─────────────────────────────────────────────────────────────────────────────────┘
```

**Button Styling:**

```css
.search-trigger {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  background: var(--bg-secondary);
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  transition: all 150ms ease;
}

.search-trigger:hover {
  background: var(--bg-tertiary);
  border-color: var(--border-default);
  color: var(--text-primary);
}

.search-trigger__shortcut {
  font-family: var(--font-mono);
  font-size: var(--text-xs);
  color: var(--text-muted);
  padding: 2px 4px;
  background: var(--bg-primary);
  border-radius: 4px;
}
```

---

## Motion & Animation

**Principles:**

- Respect `prefers-reduced-motion`
- Animate only `transform` and `opacity` (compositor-friendly)
- Keep durations short (100-200ms for micro, 200-300ms for transitions)

**Command Palette Open:**

```css
@keyframes palette-enter {
  from {
    opacity: 0;
    transform: scale(0.96) translateY(-8px);
  }
  to {
    opacity: 1;
    transform: scale(1) translateY(0);
  }
}

.command-palette {
  animation: palette-enter 150ms ease-out;
}

/* Backdrop */
.command-palette-backdrop {
  animation: fade-in 150ms ease-out;
}

@keyframes fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}
```

**Session Card Hover:**

```css
.session-card {
  transition:
    background-color 150ms ease,
    border-color 150ms ease,
    box-shadow 150ms ease;
}
```

**Stats Bar Fill:**

```css
.stats-bar__fill {
  transition: width 300ms ease-out;
}

/* Stagger animation on load */
.stats-row:nth-child(1) .stats-bar__fill { animation-delay: 0ms; }
.stats-row:nth-child(2) .stats-bar__fill { animation-delay: 50ms; }
.stats-row:nth-child(3) .stats-bar__fill { animation-delay: 100ms; }
.stats-row:nth-child(4) .stats-bar__fill { animation-delay: 150ms; }
.stats-row:nth-child(5) .stats-bar__fill { animation-delay: 200ms; }

@keyframes bar-fill {
  from { width: 0; }
}

.stats-bar__fill {
  animation: bar-fill 300ms ease-out backwards;
}
```

---

## Accessibility

Following Web Interface Guidelines:

| Requirement | Implementation |
|-------------|----------------|
| Keyboard navigation | Full ↑↓ arrow support in palette, Tab through filters |
| Focus visible | `focus-visible:ring-2 ring-offset-2 ring-blue-500` |
| Semantic HTML | `<dialog>` for modal, `<button>` for interactive, `<kbd>` for shortcuts |
| ARIA labels | Icon-only buttons have `aria-label` |
| Skip links | Command palette auto-focuses input |
| Reduced motion | All animations wrapped in `@media (prefers-reduced-motion: no-preference)` |
| Color contrast | All text meets WCAG AA (4.5:1 ratio) |

---

## Responsive Breakpoints

```css
/* Mobile first */
@media (min-width: 640px) {  /* sm */
  /* Show full search button text */
  /* Show keyboard shortcuts */
}

@media (min-width: 768px) {  /* md */
  /* Two-column layout for stats */
}

@media (min-width: 1024px) { /* lg */
  /* Full sidebar visible */
  /* Wider session cards */
}

@media (min-width: 1280px) { /* xl */
  /* Max content width 1200px */
}
```

---

## File Structure

```
src/
├── components/
│   ├── SessionCard.tsx        # Enhanced session card
│   ├── CommandPalette.tsx     # ⌘K search modal
│   ├── StatsDashboard.tsx     # Global stats view
│   └── SearchResults.tsx      # Filtered results view
├── lib/
│   ├── search.ts              # Query parser & filter logic
│   └── utils.ts               # cn() helper
└── styles/
    └── design-tokens.css      # CSS custom properties
```

---

## Summary

This design system creates a cohesive, developer-friendly experience that:

1. **Prioritizes discovery** - Stats and patterns surface before users need to search
2. **Rewards power users** - Query syntax for precise filtering
3. **Feels native** - Monospace typography, keyboard shortcuts, dark command palette
4. **Maintains polish** - Consistent spacing, subtle animations, clear hierarchy

The sage green accent creates a calm, focused aesthetic distinct from typical blue-heavy developer tools.
