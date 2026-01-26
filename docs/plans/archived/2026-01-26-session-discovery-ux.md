# Session Discovery UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform Claude View from a basic session list into a powerful discovery tool with enhanced session cards, smart search, and usage statistics.

**Architecture:** Server-side JSONL parsing extracts rich metadata (first/last messages, files touched, skills used, tool counts). Client renders enhanced cards with this data. Search uses client-side filtering with query syntax parsing. Stats are computed from aggregated session metadata.

**Tech Stack:** React 19, TypeScript, TanStack Query, Tailwind CSS, Zustand (state), Express backend

---

## Feature Overview

1. **Enhanced Session Cards** - Show first message, last message, files touched, tool usage summary
2. **Command Palette Search (⌘K)** - Query syntax with `project:`, `path:`, `skill:`, `after:`, regex support
3. **Stats Dashboard** - Global and per-project usage statistics with clickable filters
4. **Syntax Highlighting** - Highlight paths and code in previews (lower priority)

---

## Task 1: Extend Session Metadata Extraction

**Files:**
- Modify: `src/server/sessions.ts:155-201`
- Test: Manual testing via API response inspection

### Step 1: Define enhanced SessionInfo interface

Add to `src/server/sessions.ts` after line 14:

```typescript
export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: Date
  sizeBytes: number
  preview: string
  // NEW FIELDS
  lastMessage: string          // Last user message
  filesTouched: string[]       // Files edited/written (max 5)
  skillsUsed: string[]         // Slash commands detected
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
}
```

### Step 2: Rewrite getSessionPreview to extract rich metadata

Replace the `getSessionPreview` function (lines 155-201) with:

```typescript
interface SessionMetadata {
  preview: string
  lastMessage: string
  filesTouched: string[]
  skillsUsed: string[]
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
}

/**
 * Extract rich metadata from a JSONL session file
 * Scans the entire file but processes efficiently line-by-line
 */
async function getSessionMetadata(filePath: string): Promise<SessionMetadata> {
  const result: SessionMetadata = {
    preview: '(no user message found)',
    lastMessage: '',
    filesTouched: [],
    skillsUsed: [],
    toolCounts: { edit: 0, read: 0, bash: 0, write: 0 }
  }

  const filesSet = new Set<string>()
  const skillsSet = new Set<string>()
  let firstUserMessage = ''
  let lastUserMessage = ''

  let handle
  try {
    handle = await open(filePath, 'r')
    const content = await handle.readFile({ encoding: 'utf-8' })
    const lines = content.split('\n')

    for (const line of lines) {
      if (!line.trim()) continue

      try {
        const entry = JSON.parse(line)

        // Extract user messages
        if (entry.type === 'user' && entry.message?.content) {
          let text = typeof entry.message.content === 'string'
            ? entry.message.content
            : (Array.isArray(entry.message.content)
                ? entry.message.content.find((b: { type: string; text?: string }) => b.type === 'text')?.text
                : '') || ''

          // Clean command tags
          text = text.replace(/<command-[^>]*>[^<]*<\/command-[^>]*>/g, '').trim()

          // Skip system/tool messages
          if (text && !text.startsWith('<') && text.length > 10) {
            if (!firstUserMessage) {
              firstUserMessage = text.length > 200 ? text.substring(0, 200) + '…' : text
            }
            lastUserMessage = text.length > 200 ? text.substring(0, 200) + '…' : text

            // Detect skills (slash commands)
            const skillMatches = text.match(/\/[\w:-]+/g)
            if (skillMatches) {
              skillMatches.forEach(s => skillsSet.add(s))
            }
          }
        }

        // Count tool usage and extract files
        if (entry.type === 'assistant' && entry.message?.content) {
          const content = entry.message.content
          if (Array.isArray(content)) {
            for (const block of content) {
              if (block.type === 'tool_use') {
                const toolName = block.name?.toLowerCase() || ''
                if (toolName === 'edit') {
                  result.toolCounts.edit++
                  if (block.input?.file_path) filesSet.add(block.input.file_path)
                } else if (toolName === 'write') {
                  result.toolCounts.write++
                  if (block.input?.file_path) filesSet.add(block.input.file_path)
                } else if (toolName === 'read') {
                  result.toolCounts.read++
                } else if (toolName === 'bash') {
                  result.toolCounts.bash++
                }
              }
            }
          }
        }
      } catch {
        // Skip malformed lines
        continue
      }
    }

    result.preview = firstUserMessage || '(no user message found)'
    result.lastMessage = lastUserMessage
    result.filesTouched = Array.from(filesSet).slice(0, 5).map(f => {
      // Shorten to just filename for display
      const parts = f.split('/')
      return parts[parts.length - 1]
    })
    result.skillsUsed = Array.from(skillsSet).slice(0, 5)

    return result
  } catch {
    return result
  } finally {
    await handle?.close()
  }
}
```

### Step 3: Update getProjects to use new metadata

Modify the session creation block (around line 244-255) to use the new function:

```typescript
// Get rich metadata from session
const metadata = await getSessionMetadata(filePath)

sessions.push({
  id: sessionId,
  project: entry.name,
  projectPath: resolved.fullPath,
  filePath,
  modifiedAt: fileStat.mtime,
  sizeBytes: fileStat.size,
  preview: metadata.preview,
  lastMessage: metadata.lastMessage,
  filesTouched: metadata.filesTouched,
  skillsUsed: metadata.skillsUsed,
  toolCounts: metadata.toolCounts
})
```

### Step 4: Test the API response

Run: `npm run dev:server`

Open: `http://localhost:3000/api/projects`

Expected: Each session now has `lastMessage`, `filesTouched`, `skillsUsed`, `toolCounts` fields.

### Step 5: Commit

```bash
git add src/server/sessions.ts
git commit -m "feat: extract rich metadata from session files

- Add lastMessage, filesTouched, skillsUsed, toolCounts to SessionInfo
- Parse entire JSONL file for comprehensive data
- Detect slash commands from user messages
- Track Edit/Write/Read/Bash tool usage counts

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 2: Update TypeScript Types in Frontend

**Files:**
- Modify: `src/hooks/use-projects.ts`

### Step 1: Update SessionInfo interface in hook

Find the `SessionInfo` interface in `src/hooks/use-projects.ts` and update it:

```typescript
export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: string  // JSON serialized date
  sizeBytes: number
  preview: string
  lastMessage: string
  filesTouched: string[]
  skillsUsed: string[]
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
}
```

### Step 2: Commit

```bash
git add src/hooks/use-projects.ts
git commit -m "feat: update frontend SessionInfo type with rich metadata

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 3: Create Enhanced Session Card Component

**Files:**
- Create: `src/components/SessionCard.tsx`
- Modify: `src/App.tsx` (remove inline SessionCard, import new one)

### Step 1: Create the new SessionCard component

Create `src/components/SessionCard.tsx`:

```typescript
import { FileText, Terminal, Pencil, Eye } from 'lucide-react'
import { cn } from '../lib/utils'
import type { SessionInfo } from '../hooks/use-projects'

interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
  isActive: boolean
  onClick: () => void
}

function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString)
  const now = new Date()
  const diffMs = now.getTime() - date.getTime()
  const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24))

  const timeStr = date.toLocaleTimeString('en-US', {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
  })

  if (diffDays === 0) {
    return `Today, ${timeStr}`
  } else if (diffDays === 1) {
    return `Yesterday, ${timeStr}`
  } else if (diffDays < 7) {
    const dayName = date.toLocaleDateString('en-US', { weekday: 'long' })
    return `${dayName}, ${timeStr}`
  } else {
    return date.toLocaleDateString('en-US', {
      month: 'short',
      day: 'numeric',
    }) + `, ${timeStr}`
  }
}

export function SessionCard({ session, isSelected, isActive, onClick }: SessionCardProps) {
  const totalTools = session.toolCounts.edit + session.toolCounts.bash +
                     session.toolCounts.read + session.toolCounts.write

  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-4 rounded-lg border transition-colors',
        isSelected
          ? 'bg-blue-50 border-blue-500'
          : 'bg-white border-gray-200 hover:bg-gray-50 hover:border-gray-300'
      )}
    >
      {/* Header: Started message + Active indicator */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <p className="text-sm text-gray-900 line-clamp-2">
            <span className="text-gray-400 text-xs font-medium">Started:</span>{' '}
            "{session.preview}"
          </p>

          {/* Last message if different from first */}
          {session.lastMessage && session.lastMessage !== session.preview && (
            <p className="text-sm text-gray-600 line-clamp-1 mt-1">
              <span className="text-gray-400 text-xs font-medium">Ended:</span>{' '}
              "{session.lastMessage}"
            </p>
          )}
        </div>

        {isActive && (
          <span className="flex items-center gap-1 text-xs text-green-600 flex-shrink-0">
            <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            Active
          </span>
        )}
      </div>

      {/* Files touched */}
      {session.filesTouched.length > 0 && (
        <div className="flex items-center gap-1.5 mt-3 text-xs text-gray-500">
          <FileText className="w-3.5 h-3.5 text-gray-400" />
          <span className="truncate">
            {session.filesTouched.join(', ')}
          </span>
        </div>
      )}

      {/* Footer: Tool counts + Skills + Timestamp */}
      <div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-100">
        <div className="flex items-center gap-3">
          {/* Tool counts */}
          {totalTools > 0 && (
            <div className="flex items-center gap-2 text-xs text-gray-400">
              {session.toolCounts.edit > 0 && (
                <span className="flex items-center gap-0.5" title="Edits">
                  <Pencil className="w-3 h-3" />
                  {session.toolCounts.edit}
                </span>
              )}
              {session.toolCounts.bash > 0 && (
                <span className="flex items-center gap-0.5" title="Bash commands">
                  <Terminal className="w-3 h-3" />
                  {session.toolCounts.bash}
                </span>
              )}
              {session.toolCounts.read > 0 && (
                <span className="flex items-center gap-0.5" title="File reads">
                  <Eye className="w-3 h-3" />
                  {session.toolCounts.read}
                </span>
              )}
            </div>
          )}

          {/* Skills used */}
          {session.skillsUsed.length > 0 && (
            <div className="flex items-center gap-1">
              {session.skillsUsed.slice(0, 2).map(skill => (
                <span
                  key={skill}
                  className="px-1.5 py-0.5 text-xs bg-gray-100 text-gray-600 rounded font-mono"
                >
                  {skill}
                </span>
              ))}
              {session.skillsUsed.length > 2 && (
                <span className="text-xs text-gray-400">
                  +{session.skillsUsed.length - 2}
                </span>
              )}
            </div>
          )}
        </div>

        {/* Timestamp */}
        <p className="text-xs text-gray-400 tabular-nums">
          {formatRelativeTime(session.modifiedAt)}
        </p>
      </div>
    </button>
  )
}
```

### Step 2: Update App.tsx to use new component

In `src/App.tsx`:

1. Remove the inline `SessionCard` function (lines 46-80)
2. Remove `formatRelativeTime` function (lines 12-44)
3. Add import at top:

```typescript
import { SessionCard } from './components/SessionCard'
```

### Step 3: Verify the build

Run: `npm run typecheck`

Expected: No type errors

### Step 4: Test visually

Run: `npm run dev`

Expected: Session cards now show Started/Ended messages, files touched, tool counts, and skill badges.

### Step 5: Commit

```bash
git add src/components/SessionCard.tsx src/App.tsx
git commit -m "feat: enhanced session cards with rich metadata display

- Show first and last user messages
- Display files touched with icons
- Show tool usage counts (edits, bash, reads)
- Display skill badges for slash commands used
- Improved visual hierarchy and information density

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 4: Create Search Query Parser

**Files:**
- Create: `src/lib/search.ts`

### Step 1: Create the search parser module

Create `src/lib/search.ts`:

```typescript
import type { SessionInfo, ProjectInfo } from '../hooks/use-projects'

export interface ParsedQuery {
  text: string[]           // Free text terms
  project?: string         // project:name filter
  path?: string            // path:*.tsx filter (glob pattern)
  skill?: string           // skill:brainstorm filter
  after?: Date             // after:2026-01-20 filter
  before?: Date            // before:2026-01-25 filter
  regex?: RegExp           // /pattern/flags auto-detected
}

/**
 * Parse search query with syntax support:
 * - project:name
 * - path:*.tsx
 * - skill:brainstorm
 * - after:2026-01-20
 * - before:2026-01-25
 * - /regex/flags
 * - "exact phrase"
 * - plain text
 */
export function parseQuery(input: string): ParsedQuery {
  const result: ParsedQuery = { text: [] }

  if (!input.trim()) return result

  // Extract regex patterns first (e.g., /error.*fix/i)
  const regexMatch = input.match(/\/([^/]+)\/([gimsuy]*)/)
  if (regexMatch) {
    try {
      result.regex = new RegExp(regexMatch[1], regexMatch[2])
      input = input.replace(regexMatch[0], '')
    } catch {
      // Invalid regex, treat as text
    }
  }

  // Extract quoted phrases
  const phrases: string[] = []
  input = input.replace(/"([^"]+)"/g, (_, phrase) => {
    phrases.push(phrase.toLowerCase())
    return ''
  })

  // Parse tokens
  const tokens = input.trim().split(/\s+/).filter(Boolean)

  for (const token of tokens) {
    const [key, ...valueParts] = token.split(':')
    const value = valueParts.join(':')

    if (value) {
      switch (key.toLowerCase()) {
        case 'project':
          result.project = value.toLowerCase()
          break
        case 'path':
          result.path = value.toLowerCase()
          break
        case 'skill':
          result.skill = value.toLowerCase()
          break
        case 'after':
          result.after = parseDate(value)
          break
        case 'before':
          result.before = parseDate(value)
          break
        default:
          // Unknown filter, treat as text
          result.text.push(token.toLowerCase())
      }
    } else {
      result.text.push(token.toLowerCase())
    }
  }

  // Add quoted phrases to text
  result.text.push(...phrases)

  return result
}

function parseDate(value: string): Date | undefined {
  // Support formats: 2026-01-20, jan-20, yesterday, today
  const now = new Date()

  if (value === 'today') {
    return new Date(now.getFullYear(), now.getMonth(), now.getDate())
  }
  if (value === 'yesterday') {
    const d = new Date(now)
    d.setDate(d.getDate() - 1)
    return new Date(d.getFullYear(), d.getMonth(), d.getDate())
  }

  // Try ISO format
  const parsed = new Date(value)
  if (!isNaN(parsed.getTime())) {
    return parsed
  }

  // Try month-day format (e.g., jan-20)
  const monthMatch = value.match(/^([a-z]+)-(\d+)$/i)
  if (monthMatch) {
    const months = ['jan','feb','mar','apr','may','jun','jul','aug','sep','oct','nov','dec']
    const monthIndex = months.indexOf(monthMatch[1].toLowerCase())
    if (monthIndex >= 0) {
      return new Date(now.getFullYear(), monthIndex, parseInt(monthMatch[2]))
    }
  }

  return undefined
}

/**
 * Match a glob pattern against a string
 * Supports * (any chars) and ? (single char)
 */
function globMatch(pattern: string, str: string): boolean {
  const regex = pattern
    .replace(/[.+^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '.*')
    .replace(/\?/g, '.')
  return new RegExp(`^${regex}$`, 'i').test(str)
}

/**
 * Filter sessions based on parsed query
 */
export function filterSessions(
  sessions: SessionInfo[],
  projects: ProjectInfo[],
  query: ParsedQuery
): SessionInfo[] {
  return sessions.filter(session => {
    // Project filter
    if (query.project) {
      const project = projects.find(p => p.sessions.includes(session))
      if (!project) return false
      const projectName = project.displayName.toLowerCase()
      if (!projectName.includes(query.project)) return false
    }

    // Path filter (glob match against files touched)
    if (query.path) {
      const hasMatch = session.filesTouched.some(f =>
        globMatch(query.path!, f.toLowerCase())
      )
      if (!hasMatch) return false
    }

    // Skill filter
    if (query.skill) {
      const hasSkill = session.skillsUsed.some(s =>
        s.toLowerCase().includes(query.skill!)
      )
      if (!hasSkill) return false
    }

    // Date filters
    const sessionDate = new Date(session.modifiedAt)
    if (query.after && sessionDate < query.after) return false
    if (query.before && sessionDate > query.before) return false

    // Regex match against preview and lastMessage
    if (query.regex) {
      const text = `${session.preview} ${session.lastMessage}`
      if (!query.regex.test(text)) return false
    }

    // Text search (all terms must match somewhere)
    if (query.text.length > 0) {
      const searchable = `${session.preview} ${session.lastMessage} ${session.filesTouched.join(' ')} ${session.skillsUsed.join(' ')}`.toLowerCase()
      const allMatch = query.text.every(term => searchable.includes(term))
      if (!allMatch) return false
    }

    return true
  })
}
```

### Step 2: Verify the build

Run: `npm run typecheck`

Expected: No type errors

### Step 3: Commit

```bash
git add src/lib/search.ts
git commit -m "feat: add search query parser with filter syntax

- Support project:, path:, skill:, after:, before: filters
- Auto-detect /regex/ patterns
- Support quoted \"exact phrases\"
- Glob matching for path filters
- Date parsing with natural language (today, yesterday, jan-20)

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 5: Create Command Palette Component

**Files:**
- Create: `src/components/CommandPalette.tsx`

### Step 1: Create the command palette component

Create `src/components/CommandPalette.tsx`:

```typescript
import { useState, useEffect, useCallback, useRef } from 'react'
import { Search, X } from 'lucide-react'
import { cn } from '../lib/utils'

interface CommandPaletteProps {
  isOpen: boolean
  onClose: () => void
  onSearch: (query: string) => void
  recentSearches: string[]
}

export function CommandPalette({
  isOpen,
  onClose,
  onSearch,
  recentSearches
}: CommandPaletteProps) {
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  // Focus input when opened
  useEffect(() => {
    if (isOpen) {
      inputRef.current?.focus()
      setQuery('')
    }
  }, [isOpen])

  // Handle keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // ⌘K to open
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        if (!isOpen) {
          // Parent should handle opening
        }
      }
      // Escape to close
      if (e.key === 'Escape' && isOpen) {
        onClose()
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose])

  const handleSubmit = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (query.trim()) {
      onSearch(query.trim())
    }
  }, [query, onSearch])

  const handleRecentClick = useCallback((search: string) => {
    setQuery(search)
    onSearch(search)
  }, [onSearch])

  const insertFilter = useCallback((filter: string) => {
    setQuery(prev => {
      const trimmed = prev.trim()
      return trimmed ? `${trimmed} ${filter}` : filter
    })
    inputRef.current?.focus()
  }, [])

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[15vh]">
      {/* Backdrop */}
      <div
        className="absolute inset-0 bg-black/50 backdrop-blur-sm"
        onClick={onClose}
      />

      {/* Modal */}
      <div className="relative w-full max-w-xl bg-[#111113] rounded-xl shadow-2xl border border-[#2a2a2e] overflow-hidden">
        {/* Search input */}
        <form onSubmit={handleSubmit}>
          <div className="flex items-center gap-3 px-4 py-3 border-b border-[#2a2a2e]">
            <Search className="w-5 h-5 text-[#6e6e76]" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search sessions…"
              className="flex-1 bg-transparent text-[#ececef] placeholder-[#6e6e76] outline-none font-mono text-sm"
              spellCheck={false}
              autoComplete="off"
            />
            <kbd className="hidden sm:inline-flex items-center gap-1 px-2 py-0.5 text-xs text-[#6e6e76] bg-[#1c1c1f] rounded border border-[#2a2a2e]">
              ⏎
            </kbd>
            <button
              type="button"
              onClick={onClose}
              className="p-1 text-[#6e6e76] hover:text-[#ececef] transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </form>

        {/* Recent searches */}
        {recentSearches.length > 0 && (
          <div className="px-4 py-3 border-b border-[#2a2a2e]">
            <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
              Recent
            </p>
            <div className="space-y-1">
              {recentSearches.slice(0, 3).map((search, i) => (
                <button
                  key={i}
                  onClick={() => handleRecentClick(search)}
                  className="w-full flex items-center gap-2 px-2 py-1.5 text-sm text-[#ececef] hover:bg-[#1c1c1f] rounded transition-colors text-left font-mono"
                >
                  <span className="text-[#6e6e76]">○</span>
                  {search}
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Filter hints */}
        <div className="px-4 py-3">
          <p className="text-xs font-medium text-[#6e6e76] uppercase tracking-wider mb-2">
            Filters
          </p>
          <div className="flex flex-wrap gap-2">
            {['project:', 'path:', 'skill:', 'after:', '"phrase"', '/regex/'].map(filter => (
              <button
                key={filter}
                onClick={() => insertFilter(filter)}
                className="px-2 py-1 text-xs font-mono text-[#7c9885] bg-[#1c1c1f] hover:bg-[#252525] rounded border border-[#2a2a2e] transition-colors"
              >
                {filter}
              </button>
            ))}
          </div>
        </div>

        {/* Keyboard hints */}
        <div className="px-4 py-2 border-t border-[#2a2a2e] flex items-center gap-4 text-xs text-[#6e6e76]">
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">↑↓</kbd>
            Navigate
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">⏎</kbd>
            Search
          </span>
          <span className="flex items-center gap-1">
            <kbd className="px-1.5 py-0.5 bg-[#1c1c1f] rounded border border-[#2a2a2e]">⎋</kbd>
            Close
          </span>
        </div>
      </div>
    </div>
  )
}
```

### Step 2: Verify the build

Run: `npm run typecheck`

Expected: No type errors

### Step 3: Commit

```bash
git add src/components/CommandPalette.tsx
git commit -m "feat: add command palette component for search

- Dark editorial aesthetic with sage green accents
- Recent searches display
- Clickable filter hints
- Keyboard navigation hints
- Escape to close, Enter to search

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 6: Integrate Search into App

**Files:**
- Modify: `src/App.tsx`

### Step 1: Add search state and handlers

Add imports at top of `src/App.tsx`:

```typescript
import { CommandPalette } from './components/CommandPalette'
import { parseQuery, filterSessions } from './lib/search'
```

Inside the `App` component, add state:

```typescript
const [isSearchOpen, setIsSearchOpen] = useState(false)
const [searchQuery, setSearchQuery] = useState('')
const [recentSearches, setRecentSearches] = useState<string[]>(() => {
  const saved = localStorage.getItem('claude-view-recent-searches')
  return saved ? JSON.parse(saved) : []
})
```

Add keyboard handler effect:

```typescript
// ⌘K to open search
useEffect(() => {
  const handleKeyDown = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault()
      setIsSearchOpen(true)
    }
  }
  window.addEventListener('keydown', handleKeyDown)
  return () => window.removeEventListener('keydown', handleKeyDown)
}, [])
```

Add search handler:

```typescript
const handleSearch = useCallback((query: string) => {
  setSearchQuery(query)
  setIsSearchOpen(false)

  // Save to recent searches
  setRecentSearches(prev => {
    const updated = [query, ...prev.filter(s => s !== query)].slice(0, 10)
    localStorage.setItem('claude-view-recent-searches', JSON.stringify(updated))
    return updated
  })
}, [])
```

Add filtered sessions memo:

```typescript
const filteredSessions = useMemo(() => {
  if (!projects || !searchQuery) return null

  const allSessions = projects.flatMap(p => p.sessions)
  const parsed = parseQuery(searchQuery)
  return filterSessions(allSessions, projects, parsed)
}, [projects, searchQuery])
```

### Step 2: Add search button to header

In the header section, add a search button:

```typescript
<header className="h-12 bg-white border-b border-gray-200 flex items-center justify-between px-4">
  <h1 className="text-lg font-semibold text-gray-900">Claude View</h1>
  <div className="flex items-center gap-2">
    {/* Search button */}
    <button
      onClick={() => setIsSearchOpen(true)}
      className="flex items-center gap-2 px-3 py-1.5 text-sm text-gray-500 hover:text-gray-700 bg-gray-100 hover:bg-gray-200 rounded-lg transition-colors"
    >
      <Search className="w-4 h-4" />
      <span className="hidden sm:inline">Search</span>
      <kbd className="hidden sm:inline text-xs text-gray-400">⌘K</kbd>
    </button>
    {/* ... existing buttons */}
  </div>
</header>
```

Add `Search` to lucide-react imports.

### Step 3: Add CommandPalette and search results to render

Before the closing `</div>` of the App return, add:

```typescript
{/* Command Palette */}
<CommandPalette
  isOpen={isSearchOpen}
  onClose={() => setIsSearchOpen(false)}
  onSearch={handleSearch}
  recentSearches={recentSearches}
/>
```

### Step 4: Show search results when query is active

Modify MainContent rendering to show filtered results:

```typescript
{selectedSession ? (
  <ConversationView ... />
) : searchQuery && filteredSessions ? (
  <SearchResults
    sessions={filteredSessions}
    query={searchQuery}
    onSessionClick={handleSessionClick}
    onClearSearch={() => setSearchQuery('')}
  />
) : (
  <MainContent ... />
)}
```

### Step 5: Create SearchResults component inline or separate file

Add a simple SearchResults component (can be inline or in separate file):

```typescript
function SearchResults({
  sessions,
  query,
  onSessionClick,
  onClearSearch,
}: {
  sessions: SessionInfo[]
  query: string
  onSessionClick: (session: SessionInfo) => void
  onClearSearch: () => void
}) {
  return (
    <main className="flex-1 overflow-y-auto bg-gray-50 p-6">
      <div className="max-w-3xl mx-auto">
        <div className="flex items-center justify-between mb-6">
          <div>
            <h1 className="text-xl font-semibold text-gray-900">
              Search Results
            </h1>
            <p className="text-sm text-gray-500 mt-1">
              {sessions.length} sessions matching "{query}"
            </p>
          </div>
          <button
            onClick={onClearSearch}
            className="px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 bg-gray-200 hover:bg-gray-300 rounded-lg transition-colors"
          >
            Clear search
          </button>
        </div>

        <div className="space-y-3">
          {sessions.map((session) => (
            <SessionCard
              key={session.id}
              session={session}
              isSelected={false}
              isActive={false}
              onClick={() => onSessionClick(session)}
            />
          ))}
        </div>

        {sessions.length === 0 && (
          <div className="text-center py-12 text-gray-500">
            <p>No sessions match your search.</p>
            <p className="text-sm mt-1">Try different keywords or filters.</p>
          </div>
        )}
      </div>
    </main>
  )
}
```

### Step 6: Test the search

Run: `npm run dev`

Test:
1. Press ⌘K - palette opens
2. Type `project:claude-view` - Enter
3. Results filtered to that project
4. Click "Clear search" to return to normal view

### Step 7: Commit

```bash
git add src/App.tsx
git commit -m "feat: integrate command palette search into app

- ⌘K keyboard shortcut to open search
- Search button in header
- Persist recent searches to localStorage
- Show filtered results view
- Clear search to return to normal view

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 7: Create Stats Dashboard Component

**Files:**
- Create: `src/components/StatsDashboard.tsx`

### Step 1: Create the stats dashboard

Create `src/components/StatsDashboard.tsx`:

```typescript
import { useMemo } from 'react'
import { BarChart3, Zap, FolderOpen } from 'lucide-react'
import type { ProjectInfo } from '../hooks/use-projects'
import { cn } from '../lib/utils'

interface StatsDashboardProps {
  projects: ProjectInfo[]
  onFilterClick: (query: string) => void
}

export function StatsDashboard({ projects, onFilterClick }: StatsDashboardProps) {
  const stats = useMemo(() => {
    const allSessions = projects.flatMap(p => p.sessions)

    // Aggregate skills across all sessions
    const skillCounts = new Map<string, number>()
    for (const session of allSessions) {
      for (const skill of session.skillsUsed) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
    }
    const topSkills = Array.from(skillCounts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5)

    // Find max for bar scaling
    const maxSkillCount = topSkills[0]?.[1] || 1

    // Project stats sorted by session count
    const projectStats = projects
      .map(p => ({
        name: p.displayName,
        fullName: p.name,
        sessions: p.sessions.length,
        activeCount: p.activeCount,
      }))
      .sort((a, b) => b.sessions - a.sessions)
      .slice(0, 5)

    const maxProjectSessions = projectStats[0]?.sessions || 1

    // Find earliest session
    const earliest = allSessions.reduce((min, s) => {
      const d = new Date(s.modifiedAt)
      return d < min ? d : min
    }, new Date())

    return {
      totalSessions: allSessions.length,
      totalProjects: projects.length,
      since: earliest.toLocaleDateString('en-US', { month: 'short', year: 'numeric' }),
      topSkills,
      maxSkillCount,
      projectStats,
      maxProjectSessions,
    }
  }, [projects])

  return (
    <div className="bg-white rounded-xl border border-gray-200 p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center gap-2">
        <BarChart3 className="w-5 h-5 text-gray-400" />
        <h2 className="text-lg font-semibold text-gray-900">Your Usage</h2>
      </div>

      {/* Overview stats */}
      <div className="flex items-center gap-4 text-sm text-gray-600">
        <span className="tabular-nums font-medium">{stats.totalSessions}</span> sessions
        <span className="text-gray-300">·</span>
        <span className="tabular-nums font-medium">{stats.totalProjects}</span> projects
        <span className="text-gray-300">·</span>
        since {stats.since}
      </div>

      {/* Top skills */}
      {stats.topSkills.length > 0 && (
        <div>
          <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3 flex items-center gap-1.5">
            <Zap className="w-3.5 h-3.5" />
            Top Skills
          </h3>
          <div className="space-y-2">
            {stats.topSkills.map(([skill, count]) => (
              <button
                key={skill}
                onClick={() => onFilterClick(`skill:${skill.replace('/', '')}`)}
                className="w-full group"
              >
                <div className="flex items-center justify-between text-sm mb-1">
                  <span className="font-mono text-gray-700 group-hover:text-blue-600 transition-colors">
                    {skill}
                  </span>
                  <span className="tabular-nums text-gray-400">{count}</span>
                </div>
                <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-[#7c9885] group-hover:bg-blue-500 transition-colors rounded-full"
                    style={{ width: `${(count / stats.maxSkillCount) * 100}%` }}
                  />
                </div>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Top projects */}
      <div>
        <h3 className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-3 flex items-center gap-1.5">
          <FolderOpen className="w-3.5 h-3.5" />
          Most Active Projects
        </h3>
        <div className="space-y-2">
          {stats.projectStats.map((project) => (
            <button
              key={project.fullName}
              onClick={() => onFilterClick(`project:${project.name}`)}
              className="w-full group"
            >
              <div className="flex items-center justify-between text-sm mb-1">
                <span className="flex items-center gap-2">
                  <span className="text-gray-700 group-hover:text-blue-600 transition-colors">
                    {project.name}
                  </span>
                  {project.activeCount > 0 && (
                    <span className="flex items-center gap-1 text-xs text-green-600">
                      <span className="w-1.5 h-1.5 bg-green-500 rounded-full" />
                      {project.activeCount}
                    </span>
                  )}
                </span>
                <span className="tabular-nums text-gray-400">{project.sessions}</span>
              </div>
              <div className="h-1.5 bg-gray-100 rounded-full overflow-hidden">
                <div
                  className={cn(
                    "h-full rounded-full transition-colors",
                    project.activeCount > 0
                      ? "bg-green-400 group-hover:bg-green-500"
                      : "bg-gray-300 group-hover:bg-blue-500"
                  )}
                  style={{ width: `${(project.sessions / stats.maxProjectSessions) * 100}%` }}
                />
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  )
}
```

### Step 2: Integrate into App

Add to imports in `src/App.tsx`:

```typescript
import { StatsDashboard } from './components/StatsDashboard'
```

Add dashboard to the main content area (e.g., when no project selected or as sidebar):

One approach - show dashboard when no project selected:

```typescript
function MainContent({ ... }) {
  if (!selectedProject) {
    return (
      <main className="flex-1 overflow-y-auto bg-gray-50 p-6">
        <div className="max-w-3xl mx-auto">
          <StatsDashboard
            projects={projects}
            onFilterClick={(query) => {
              // This needs to be passed from parent
              // For now, show how to wire it up
            }}
          />
        </div>
      </main>
    )
  }
  // ... rest of MainContent
}
```

### Step 3: Commit

```bash
git add src/components/StatsDashboard.tsx src/App.tsx
git commit -m "feat: add stats dashboard with skill and project insights

- Show total sessions, projects, and usage timeline
- Display top skills with clickable bars
- Show most active projects with session counts
- Clicking stats triggers search filter

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 8: Add Per-Project Stats to Sidebar

**Files:**
- Modify: `src/App.tsx` (Sidebar component)

### Step 1: Enhance Sidebar with project stats

When a project is selected, show mini-stats below the project list:

```typescript
function Sidebar({
  projects,
  selectedProject,
  onProjectClick,
  onFilterClick,
}: {
  projects: ProjectInfo[]
  selectedProject: string | null
  onProjectClick: (project: ProjectInfo) => void
  onFilterClick: (query: string) => void
}) {
  const selectedProjectData = projects.find(p => p.name === selectedProject)

  // Calculate per-project stats
  const projectStats = useMemo(() => {
    if (!selectedProjectData) return null

    const skillCounts = new Map<string, number>()
    const fileCounts = new Map<string, number>()

    for (const session of selectedProjectData.sessions) {
      for (const skill of session.skillsUsed) {
        skillCounts.set(skill, (skillCounts.get(skill) || 0) + 1)
      }
      for (const file of session.filesTouched) {
        fileCounts.set(file, (fileCounts.get(file) || 0) + 1)
      }
    }

    return {
      topSkills: Array.from(skillCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3),
      topFiles: Array.from(fileCounts.entries())
        .sort((a, b) => b[1] - a[1])
        .slice(0, 3),
    }
  }, [selectedProjectData])

  return (
    <aside className="w-72 bg-gray-50/80 border-r border-gray-200 flex flex-col overflow-hidden">
      {/* Project list */}
      <div className="flex-1 overflow-y-auto py-2">
        {/* ... existing project list ... */}
      </div>

      {/* Per-project stats */}
      {projectStats && (
        <div className="border-t border-gray-200 p-3 space-y-3">
          {projectStats.topSkills.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-1.5">
                Skills
              </p>
              <div className="flex flex-wrap gap-1">
                {projectStats.topSkills.map(([skill, count]) => (
                  <button
                    key={skill}
                    onClick={() => onFilterClick(`project:${selectedProjectData!.displayName} skill:${skill.replace('/', '')}`)}
                    className="px-1.5 py-0.5 text-[11px] font-mono bg-gray-200 hover:bg-gray-300 text-gray-600 rounded transition-colors"
                  >
                    {skill} <span className="text-gray-400">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {projectStats.topFiles.length > 0 && (
            <div>
              <p className="text-[10px] font-medium text-gray-400 uppercase tracking-wider mb-1.5">
                Top Files
              </p>
              <div className="space-y-0.5">
                {projectStats.topFiles.map(([file, count]) => (
                  <button
                    key={file}
                    onClick={() => onFilterClick(`project:${selectedProjectData!.displayName} path:${file}`)}
                    className="w-full flex items-center justify-between px-1.5 py-0.5 text-[11px] hover:bg-gray-200 rounded transition-colors"
                  >
                    <span className="truncate text-gray-600">{file}</span>
                    <span className="text-gray-400 tabular-nums">{count}</span>
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </aside>
  )
}
```

### Step 2: Wire up onFilterClick through App

Pass the handler to Sidebar:

```typescript
<Sidebar
  projects={projects}
  selectedProject={selectedProjectName}
  onProjectClick={handleProjectClick}
  onFilterClick={(query) => {
    setSearchQuery(query)
  }}
/>
```

### Step 3: Commit

```bash
git add src/App.tsx
git commit -m "feat: add per-project stats in sidebar

- Show top skills used in selected project
- Show most frequently touched files
- Clickable items trigger filtered search

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Summary

This plan implements:

1. **Enhanced Session Cards** - Rich metadata extraction and display
2. **Command Palette Search** - ⌘K triggered with query syntax
3. **Stats Dashboard** - Global usage insights
4. **Per-Project Stats** - Sidebar stats when project selected

All features use clickable elements as search entry points, making discovery intuitive.

---

**Plan complete and saved to `docs/plans/2026-01-26-session-discovery-ux.md`. Two execution options:**

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

**Which approach?**