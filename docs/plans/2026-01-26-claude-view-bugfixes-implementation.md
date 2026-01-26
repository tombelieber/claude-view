# Claude View Bugfixes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix 6 bugs found via E2E testing: data pipeline issues, remove unreliable active indicators, add virtualization, and improve XML rendering.

**Architecture:** Debug backend data extraction first (root cause), then fix frontend display issues. Add react-virtuoso for chat performance. Create XmlCard component for progressive disclosure.

**Tech Stack:** React, TypeScript, react-virtuoso, react-router-dom, Tailwind CSS

---

## Task 1: Verify Data Pipeline - Backend Investigation

**Files:**
- Read: `src/server/sessions.ts:179-274` (getSessionMetadata function)
- Read: `src/server/sessions.ts:279-375` (getProjects function)

**Step 1: Add debug logging to getSessionMetadata**

In `src/server/sessions.ts`, temporarily add console logs to verify data extraction:

```typescript
// At line 258, before return statement in getSessionMetadata
console.log('[DEBUG] Session metadata:', {
  preview: result.preview.substring(0, 50),
  lastMessage: result.lastMessage.substring(0, 50),
  filesTouched: result.filesTouched.length,
  skillsUsed: result.skillsUsed,
  toolCounts: result.toolCounts
})
```

**Step 2: Test with dev server**

Run: `npm run dev`
Expected: Console logs showing extracted metadata for each session

**Step 3: Identify the bug**

Check console output:
- If `toolCounts` shows real numbers → data extraction works, bug is in frontend
- If `toolCounts` all 0 → bug is in JSONL parsing logic

**Step 4: Remove debug logging after verification**

Remove the console.log statement added in Step 1.

---

## Task 2: Fix Tool Count Extraction (if needed)

**Files:**
- Modify: `src/server/sessions.ts:231-248`

**Step 1: Check tool_use block structure**

The current code checks `block.name?.toLowerCase()` but JSONL may use different casing.

```typescript
// Current (line 237-248):
if (block.type === 'tool_use') {
  const toolName = block.name?.toLowerCase() || ''
  if (toolName === 'edit') {
    result.toolCounts.edit++
    if (block.input?.file_path) filesSet.add(block.input.file_path)
  } else if (toolName === 'write') {
    // ...
```

**Step 2: Add case-insensitive matching with logging**

```typescript
if (block.type === 'tool_use') {
  const toolName = (block.name || '').toLowerCase()

  // Match Edit/edit, Write/write, Read/read, Bash/bash
  if (toolName === 'edit' || toolName === 'Edit') {
    result.toolCounts.edit++
    if (block.input?.file_path) filesSet.add(block.input.file_path)
  } else if (toolName === 'write' || toolName === 'Write') {
    result.toolCounts.write++
    if (block.input?.file_path) filesSet.add(block.input.file_path)
  } else if (toolName === 'read' || toolName === 'Read') {
    result.toolCounts.read++
  } else if (toolName === 'bash' || toolName === 'Bash') {
    result.toolCounts.bash++
  }
}
```

**Step 3: Verify fix**

Run: `npm run dev`
Check: Sidebar tool stats now show non-zero values

**Step 4: Commit**

```bash
git add src/server/sessions.ts
git commit -m "fix: improve tool count extraction case handling

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 3: Add messageCount and turnCount to API

**Files:**
- Modify: `src/server/sessions.ts:162-173` (SessionMetadata interface)
- Modify: `src/server/sessions.ts:179-274` (getSessionMetadata function)
- Modify: `src/server/sessions.ts:6-24` (SessionInfo interface)
- Modify: `src/hooks/use-projects.ts:3-20` (SessionInfo interface)

**Step 1: Update SessionMetadata interface**

In `src/server/sessions.ts` at line 162:

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
  messageCount: number  // ADD
  turnCount: number     // ADD
}
```

**Step 2: Update getSessionMetadata initialization**

In `src/server/sessions.ts` at line 180:

```typescript
const result: SessionMetadata = {
  preview: '(no user message found)',
  lastMessage: '',
  filesTouched: [],
  skillsUsed: [],
  toolCounts: { edit: 0, read: 0, bash: 0, write: 0 },
  messageCount: 0,  // ADD
  turnCount: 0      // ADD
}
```

**Step 3: Count messages and turns in parsing loop**

In `src/server/sessions.ts`, add counters after line 191:

```typescript
let userMessageCount = 0
let assistantMessageCount = 0
```

Then inside the parsing loop, after extracting user messages (around line 217):

```typescript
if (text && !text.startsWith('<') && text.length > 10) {
  userMessageCount++  // ADD
  // ... existing code
}
```

And for assistant messages (add new block around line 232):

```typescript
// Count assistant messages
if (entry.type === 'assistant' && entry.message?.content) {
  assistantMessageCount++  // ADD
  // ... existing tool extraction code
}
```

**Step 4: Calculate final counts before return**

Before `return result` around line 267:

```typescript
result.messageCount = userMessageCount + assistantMessageCount
result.turnCount = Math.min(userMessageCount, assistantMessageCount)
```

**Step 5: Update SessionInfo interface**

In `src/server/sessions.ts` at line 6:

```typescript
export interface SessionInfo {
  id: string
  project: string
  projectPath: string
  filePath: string
  modifiedAt: Date
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
  messageCount: number  // ADD
  turnCount: number     // ADD
}
```

**Step 6: Pass new fields in getProjects**

In `src/server/sessions.ts` around line 320, add to sessions.push():

```typescript
sessions.push({
  // ... existing fields
  toolCounts: metadata.toolCounts,
  messageCount: metadata.messageCount,  // ADD
  turnCount: metadata.turnCount         // ADD
})
```

**Step 7: Update frontend SessionInfo type**

In `src/hooks/use-projects.ts`:

```typescript
export interface SessionInfo {
  // ... existing fields
  toolCounts: {
    edit: number
    read: number
    bash: number
    write: number
  }
  messageCount: number  // ADD
  turnCount: number     // ADD
}
```

**Step 8: Verify types compile**

Run: `npx tsc --noEmit`
Expected: No type errors

**Step 9: Commit**

```bash
git add src/server/sessions.ts src/hooks/use-projects.ts
git commit -m "feat: add messageCount and turnCount to session API

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 4: Remove Active Session Indicators

**Files:**
- Modify: `src/components/SessionCard.tsx:5-9,51,71-76`
- Modify: `src/components/Sidebar.tsx:80,110-124,149-153`
- Modify: `src/components/StatsDashboard.tsx:43,151-156,162-165`

**Step 1: Update SessionCard props interface**

In `src/components/SessionCard.tsx` at line 5:

```typescript
interface SessionCardProps {
  session: SessionInfo
  isSelected: boolean
  // REMOVE: isActive: boolean
  onClick: () => void
}
```

**Step 2: Update SessionCard function signature**

At line 39:

```typescript
export function SessionCard({ session, isSelected, onClick }: SessionCardProps) {
```

**Step 3: Remove active border class**

At line 51, remove the isActive conditional:

```typescript
// BEFORE:
isActive && 'border-l-2 border-l-green-500'

// AFTER: Remove this entire line
```

**Step 4: Remove active indicator JSX**

Remove lines 71-76 entirely:

```typescript
// REMOVE THIS BLOCK:
{isActive && (
  <span className="flex items-center gap-1 text-xs text-green-600 flex-shrink-0">
    <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
    Active
  </span>
)}
```

**Step 5: Update Sidebar - remove hasActive variable**

In `src/components/Sidebar.tsx` at line 80, remove:

```typescript
// REMOVE:
const hasActive = project.activeCount > 0
```

**Step 6: Update Sidebar - remove active indicator in project list**

Remove lines 110-124 (the hasActive conditional rendering):

```typescript
// REMOVE THIS BLOCK:
{hasActive && (
  <span className="flex items-center gap-1">
    <span className={cn(
      'w-1.5 h-1.5 rounded-full animate-pulse',
      isSelected ? 'bg-green-300' : 'bg-green-500'
    )} />
    <span className={cn(
      'text-xs tabular-nums',
      isSelected ? 'text-green-200' : 'text-green-600'
    )}>
      {project.activeCount}
    </span>
  </span>
)}
```

**Step 7: Update Sidebar - remove active count in stats panel**

At lines 149-153, remove the active count display:

```typescript
// BEFORE:
<p className="text-xs text-gray-500 mt-1">
  {selectedProject.activeCount > 0 && (
    <span className="text-green-600">
      ●{selectedProject.activeCount} active ·
    </span>
  )}
  {selectedProject.sessions.length} sessions
</p>

// AFTER:
<p className="text-xs text-gray-500 mt-1">
  {selectedProject.sessions.length} sessions
</p>
```

**Step 8: Update StatsDashboard - remove active indicators**

In `src/components/StatsDashboard.tsx`, remove activeCount from projectStats (line 43):

```typescript
// BEFORE:
const projectStats = projects
  .map(p => ({
    name: p.displayName,
    fullName: p.name,
    sessions: p.sessions.length,
    activeCount: p.activeCount,  // REMOVE
  }))

// AFTER:
const projectStats = projects
  .map(p => ({
    name: p.displayName,
    fullName: p.name,
    sessions: p.sessions.length,
  }))
```

**Step 9: Remove active indicator in project list (StatsDashboard)**

Remove lines 151-156:

```typescript
// REMOVE:
{project.activeCount > 0 && (
  <span className="flex items-center gap-1 text-xs text-green-600">
    <span className="w-1.5 h-1.5 bg-green-500 rounded-full animate-pulse" />
    {project.activeCount}
  </span>
)}
```

**Step 10: Simplify bar color logic (StatsDashboard)**

At lines 162-165:

```typescript
// BEFORE:
className={cn(
  "h-full rounded-full transition-colors",
  project.activeCount > 0
    ? "bg-green-400 group-hover:bg-green-500"
    : "bg-gray-300 group-hover:bg-blue-500"
)}

// AFTER:
className="h-full rounded-full transition-colors bg-gray-300 group-hover:bg-blue-500"
```

**Step 11: Update any parent components passing isActive**

Search for `isActive` prop usage and remove it from SessionCard calls.

**Step 12: Verify build**

Run: `npm run build`
Expected: No errors

**Step 13: Commit**

```bash
git add src/components/SessionCard.tsx src/components/Sidebar.tsx src/components/StatsDashboard.tsx
git commit -m "feat: remove unreliable active session indicators

Removes 5-minute heuristic active detection. Leaves data model intact
for future real-time CLI integration.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 5: Enhance SessionCard Display

**Files:**
- Modify: `src/components/SessionCard.tsx`

**Step 1: Add MessageSquare icon import**

At line 1:

```typescript
import { FileText, Terminal, Pencil, Eye, MessageSquare } from 'lucide-react'
```

**Step 2: Update card layout with message count**

Replace the footer section (lines 89-140) with enhanced layout:

```typescript
{/* Footer: Tool counts + Message stats + Skills + Timestamp */}
<div className="flex items-center justify-between mt-3 pt-3 border-t border-gray-100">
  <div className="flex items-center gap-3">
    {/* Tool counts */}
    <div className="flex items-center gap-2 text-xs text-gray-400">
      {(toolCounts.edit > 0 || toolCounts.write > 0) && (
        <span className="flex items-center gap-0.5" title="Edits">
          <Pencil className="w-3 h-3" />
          <span className="tabular-nums">{toolCounts.edit + toolCounts.write}</span>
        </span>
      )}
      {toolCounts.bash > 0 && (
        <span className="flex items-center gap-0.5" title="Bash commands">
          <Terminal className="w-3 h-3" />
          <span className="tabular-nums">{toolCounts.bash}</span>
        </span>
      )}
      {toolCounts.read > 0 && (
        <span className="flex items-center gap-0.5" title="File reads">
          <Eye className="w-3 h-3" />
          <span className="tabular-nums">{toolCounts.read}</span>
        </span>
      )}
    </div>

    {/* Message count and turns */}
    {(session.messageCount ?? 0) > 0 && (
      <span className="flex items-center gap-1 text-xs text-gray-400" title="Messages and conversation turns">
        <MessageSquare className="w-3 h-3" />
        <span className="tabular-nums">{session.messageCount} msgs</span>
        {(session.turnCount ?? 0) > 0 && (
          <span className="text-gray-300">·</span>
        )}
        {(session.turnCount ?? 0) > 0 && (
          <span className="tabular-nums">{session.turnCount} turns</span>
        )}
      </span>
    )}

    {/* Skills used */}
    {(session.skillsUsed?.length ?? 0) > 0 && (
      <div className="flex items-center gap-1">
        {session.skillsUsed?.slice(0, 2).map(skill => (
          <span
            key={skill}
            className="px-1.5 py-0.5 text-xs bg-gray-100 text-gray-600 rounded font-mono"
          >
            {skill}
          </span>
        ))}
        {(session.skillsUsed?.length ?? 0) > 2 && (
          <span className="text-xs text-gray-400">
            +{(session.skillsUsed?.length ?? 0) - 2}
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
```

**Step 3: Update Started/Ended display with arrow**

Replace the header section with cleaner layout:

```typescript
{/* Header: Started message */}
<div className="flex items-start justify-between gap-2">
  <div className="flex-1 min-w-0">
    <p className="text-sm font-medium text-gray-900 line-clamp-2">
      "{session.preview}"
    </p>

    {/* Last message with arrow prefix */}
    {session.lastMessage && session.lastMessage !== session.preview && (
      <p className="text-sm text-gray-500 line-clamp-1 mt-1">
        <span className="text-gray-400">→</span> "{session.lastMessage}"
      </p>
    )}
  </div>

  {/* Timestamp (moved to top-right) */}
  <span className="text-xs text-gray-400 tabular-nums flex-shrink-0">
    {formatRelativeTime(session.modifiedAt)}
  </span>
</div>
```

**Step 4: Remove duplicate timestamp from footer**

Since timestamp is now in header, remove from footer.

**Step 5: Verify build**

Run: `npm run build`
Expected: No errors

**Step 6: Commit**

```bash
git add src/components/SessionCard.tsx
git commit -m "feat: enhance SessionCard with message count and cleaner layout

- Add message count and turn count display
- Move timestamp to top-right for better scanning
- Use arrow prefix for ending message
- Combine edit+write counts

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 6: Install react-virtuoso

**Files:**
- Modify: `package.json`

**Step 1: Install dependency**

Run: `npm install react-virtuoso`

**Step 2: Verify installation**

Run: `npm ls react-virtuoso`
Expected: Shows react-virtuoso version

**Step 3: Commit**

```bash
git add package.json package-lock.json
git commit -m "chore: add react-virtuoso for chat virtualization

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 7: Implement Virtualized ConversationView

**Files:**
- Modify: `src/components/ConversationView.tsx`

**Step 1: Add Virtuoso import**

At line 1:

```typescript
import { Virtuoso } from 'react-virtuoso'
```

**Step 2: Replace messages map with Virtuoso**

Replace lines 96-114 with:

```typescript
{/* Messages - Virtualized */}
<Virtuoso
  data={session.messages}
  itemContent={(index, message) => (
    <div className="max-w-4xl mx-auto px-6">
      <Message key={index} message={message} />
    </div>
  )}
  className="flex-1"
  followOutput="smooth"
  increaseViewportBy={{ top: 400, bottom: 400 }}
  alignToBottom={false}
  initialTopMostItemIndex={0}
  components={{
    Footer: () => (
      session.messages.length > 0 ? (
        <div className="max-w-4xl mx-auto px-6 py-6">
          <div className="text-center text-sm text-gray-400">
            {session.metadata.totalMessages} messages
            {session.metadata.toolCallCount > 0 && (
              <> &bull; {session.metadata.toolCallCount} tool calls</>
            )}
          </div>
        </div>
      ) : null
    )
  }}
  style={{ height: '100%' }}
/>
```

**Step 3: Update container styling**

Ensure the parent container has proper height:

```typescript
<main className="flex-1 flex flex-col overflow-hidden bg-gray-50">
  {/* Header unchanged */}

  {/* Messages container - remove inner overflow, let Virtuoso handle it */}
  <div className="flex-1 overflow-hidden">
    <Virtuoso ... />
  </div>
</main>
```

**Step 4: Add spacing between messages**

Update itemContent to include spacing:

```typescript
itemContent={(index, message) => (
  <div className="max-w-4xl mx-auto px-6 py-2">
    <Message key={index} message={message} />
  </div>
)}
```

**Step 5: Test with long conversation**

Run: `npm run dev`
Navigate to a conversation with 50+ messages
Expected: Fast initial load, smooth scrolling

**Step 6: Commit**

```bash
git add src/components/ConversationView.tsx
git commit -m "perf: add react-virtuoso for chat message virtualization

Only renders visible messages plus buffer for smooth scrolling.
Handles variable height messages automatically.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 8: Create XmlCard Component

**Files:**
- Create: `src/components/XmlCard.tsx`

**Step 1: Create the component file**

```typescript
import { useState } from 'react'
import { ChevronRight, ChevronDown, FileText, Brain, Wrench, FileCode } from 'lucide-react'
import { cn } from '../lib/utils'
import { CodeBlock } from './CodeBlock'

interface XmlCardProps {
  content: string
  type: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'unknown'
}

interface ParsedObservation {
  type?: string
  title?: string
  subtitle?: string
  facts?: string[]
  narrative?: string
  filesRead?: string[]
  filesModified?: string[]
}

interface ParsedToolCall {
  whatHappened?: string
  parameters?: string
  outcome?: string
  workingDirectory?: string
}

function parseObservation(xml: string): ParsedObservation {
  const result: ParsedObservation = {}

  const typeMatch = xml.match(/<type>([^<]+)<\/type>/)
  if (typeMatch) result.type = typeMatch[1]

  const titleMatch = xml.match(/<title>([^<]+)<\/title>/)
  if (titleMatch) result.title = titleMatch[1]

  const subtitleMatch = xml.match(/<subtitle>([^<]+)<\/subtitle>/)
  if (subtitleMatch) result.subtitle = subtitleMatch[1]

  const factsMatch = xml.match(/<facts>([\s\S]*?)<\/facts>/)
  if (factsMatch) {
    const factMatches = factsMatch[1].match(/<fact>([^<]+)<\/fact>/g)
    if (factMatches) {
      result.facts = factMatches.map(f => f.replace(/<\/?fact>/g, ''))
    }
  }

  const narrativeMatch = xml.match(/<narrative>([\s\S]*?)<\/narrative>/)
  if (narrativeMatch) result.narrative = narrativeMatch[1].trim()

  const filesReadMatch = xml.match(/<files_read>([\s\S]*?)<\/files_read>/)
  if (filesReadMatch) {
    const fileMatches = filesReadMatch[1].match(/<file>([^<]+)<\/file>/g)
    if (fileMatches) {
      result.filesRead = fileMatches.map(f => f.replace(/<\/?file>/g, ''))
    }
  }

  return result
}

function parseToolCall(xml: string): ParsedToolCall {
  const result: ParsedToolCall = {}

  const whatMatch = xml.match(/<what_happened>([^<]+)<\/what_happened>/)
  if (whatMatch) result.whatHappened = whatMatch[1]

  const paramsMatch = xml.match(/<parameters>"?([^"<]+)"?<\/parameters>/)
  if (paramsMatch) {
    try {
      const parsed = JSON.parse(paramsMatch[1])
      result.parameters = parsed.file_path || JSON.stringify(parsed).substring(0, 100)
    } catch {
      result.parameters = paramsMatch[1].substring(0, 100)
    }
  }

  const dirMatch = xml.match(/<working_directory>([^<]+)<\/working_directory>/)
  if (dirMatch) result.workingDirectory = dirMatch[1]

  return result
}

function getIcon(type: XmlCardProps['type']) {
  switch (type) {
    case 'observed_from_primary_session':
      return FileText
    case 'observation':
      return Brain
    case 'tool_call':
      return Wrench
    default:
      return FileCode
  }
}

function getLabel(type: XmlCardProps['type']) {
  switch (type) {
    case 'observed_from_primary_session':
      return 'Tool Call'
    case 'observation':
      return 'Observation'
    case 'tool_call':
      return 'Tool'
    default:
      return 'Structured Content'
  }
}

export function XmlCard({ content, type }: XmlCardProps) {
  const [expanded, setExpanded] = useState(false)

  const Icon = getIcon(type)
  const label = getLabel(type)

  // Parse based on type
  let summary = ''
  let details: React.ReactNode = null

  if (type === 'observed_from_primary_session') {
    const parsed = parseToolCall(content)
    summary = `${parsed.whatHappened || 'Action'}`
    if (parsed.parameters) {
      const filename = parsed.parameters.split('/').pop() || parsed.parameters
      summary += ` · ${filename}`
    }

    details = (
      <div className="space-y-2 text-sm">
        {parsed.workingDirectory && (
          <p className="text-gray-500 font-mono text-xs truncate">
            {parsed.workingDirectory}
          </p>
        )}
        {parsed.parameters && (
          <p className="text-gray-600">
            <span className="text-gray-400">Path:</span> {parsed.parameters}
          </p>
        )}
      </div>
    )
  } else if (type === 'observation') {
    const parsed = parseObservation(content)
    summary = `${parsed.type || 'Discovery'} · ${parsed.title || 'Observation'}`

    details = (
      <div className="space-y-3 text-sm">
        {parsed.subtitle && (
          <p className="text-gray-600 italic">{parsed.subtitle}</p>
        )}
        {parsed.facts && parsed.facts.length > 0 && (
          <div>
            <p className="text-xs text-gray-400 uppercase tracking-wider mb-1">Key facts:</p>
            <ul className="list-disc pl-4 space-y-0.5 text-gray-600">
              {parsed.facts.slice(0, expanded ? undefined : 3).map((fact, i) => (
                <li key={i}>{fact}</li>
              ))}
              {!expanded && parsed.facts.length > 3 && (
                <li className="text-gray-400">+{parsed.facts.length - 3} more...</li>
              )}
            </ul>
          </div>
        )}
        {parsed.filesRead && parsed.filesRead.length > 0 && (
          <p className="text-gray-500 text-xs">
            Files: {parsed.filesRead.join(', ')}
          </p>
        )}
      </div>
    )
  } else {
    // Unknown XML - show as code block
    summary = 'Structured content'
    details = (
      <CodeBlock code={content} language="xml" />
    )
  }

  return (
    <div className="border border-gray-200 rounded-lg overflow-hidden bg-white my-2">
      {/* Header - always visible */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-50 transition-colors"
      >
        <Icon className="w-4 h-4 text-gray-400 flex-shrink-0" />
        <span className="text-sm text-gray-600 truncate flex-1">
          {summary}
        </span>
        {expanded ? (
          <ChevronDown className="w-4 h-4 text-gray-400" />
        ) : (
          <ChevronRight className="w-4 h-4 text-gray-400" />
        )}
      </button>

      {/* Expanded content */}
      {expanded && (
        <div className="px-3 py-2 border-t border-gray-100 bg-gray-50">
          {details}
        </div>
      )}
    </div>
  )
}

/**
 * Detect if content contains XML that should be rendered as a card
 */
export function detectXmlType(content: string): XmlCardProps['type'] | null {
  if (content.includes('<observed_from_primary_session>')) {
    return 'observed_from_primary_session'
  }
  if (content.includes('<observation>')) {
    return 'observation'
  }
  if (content.includes('<tool_call>')) {
    return 'tool_call'
  }
  // Check for any XML-like structure
  if (/<[a-z_]+>[\s\S]*<\/[a-z_]+>/i.test(content) && content.length > 100) {
    return 'unknown'
  }
  return null
}

/**
 * Extract XML blocks from content
 */
export function extractXmlBlocks(content: string): { xml: string; type: XmlCardProps['type'] }[] {
  const blocks: { xml: string; type: XmlCardProps['type'] }[] = []

  const patterns = [
    { regex: /<observed_from_primary_session>[\s\S]*?<\/observed_from_primary_session>/g, type: 'observed_from_primary_session' as const },
    { regex: /<observation>[\s\S]*?<\/observation>/g, type: 'observation' as const },
  ]

  for (const { regex, type } of patterns) {
    const matches = content.match(regex)
    if (matches) {
      for (const match of matches) {
        blocks.push({ xml: match, type })
      }
    }
  }

  return blocks
}
```

**Step 2: Commit**

```bash
git add src/components/XmlCard.tsx
git commit -m "feat: create XmlCard component for progressive disclosure

Renders structured XML content as collapsible cards with:
- Tool call cards (Read, Edit, etc.)
- Observation cards with facts and narrative
- Fallback code block for unknown XML

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 9: Integrate XmlCard into Message Component

**Files:**
- Modify: `src/components/Message.tsx`

**Step 1: Add XmlCard imports**

At line 5:

```typescript
import { XmlCard, detectXmlType, extractXmlBlocks } from './XmlCard'
```

**Step 2: Create content preprocessor**

Add before the Message component:

```typescript
/**
 * Process message content to extract XML blocks
 * Returns segments of text and XML for mixed rendering
 */
function processContent(content: string): Array<{ type: 'text' | 'xml'; content: string; xmlType?: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'unknown' }> {
  const xmlBlocks = extractXmlBlocks(content)

  if (xmlBlocks.length === 0) {
    return [{ type: 'text', content }]
  }

  const segments: Array<{ type: 'text' | 'xml'; content: string; xmlType?: 'observed_from_primary_session' | 'observation' | 'tool_call' | 'unknown' }> = []
  let remaining = content

  for (const block of xmlBlocks) {
    const index = remaining.indexOf(block.xml)
    if (index > 0) {
      const textBefore = remaining.substring(0, index).trim()
      if (textBefore) {
        segments.push({ type: 'text', content: textBefore })
      }
    }
    segments.push({ type: 'xml', content: block.xml, xmlType: block.type })
    remaining = remaining.substring(index + block.xml.length)
  }

  const textAfter = remaining.trim()
  if (textAfter) {
    segments.push({ type: 'text', content: textAfter })
  }

  return segments
}
```

**Step 3: Update Message rendering**

Replace the content rendering section (around line 60):

```typescript
{/* Content */}
<div className="pl-11">
  {processContent(message.content).map((segment, i) => {
    if (segment.type === 'xml' && segment.xmlType) {
      return (
        <XmlCard
          key={i}
          content={segment.content}
          type={segment.xmlType}
        />
      )
    }

    return (
      <div key={i} className="prose prose-sm prose-gray max-w-none break-words">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
            // ... existing component overrides
          }}
        >
          {segment.content}
        </ReactMarkdown>
      </div>
    )
  })}

  {/* Tool calls badge */}
  {message.toolCalls && message.toolCalls.length > 0 && (
    <ToolBadge toolCalls={message.toolCalls} />
  )}
</div>
```

**Step 4: Verify build**

Run: `npm run build`
Expected: No errors

**Step 5: Test with XML content**

Run: `npm run dev`
Navigate to a conversation with XML tool outputs
Expected: XML renders as collapsible cards

**Step 6: Commit**

```bash
git add src/components/Message.tsx
git commit -m "feat: integrate XmlCard into Message rendering

Preprocesses message content to extract XML blocks and render
them as collapsible cards while preserving markdown for text.

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 10: Update Parent Components for SessionCard Props

**Files:**
- Modify: Any component that renders SessionCard (search for `<SessionCard`)

**Step 1: Find all SessionCard usages**

Run: `grep -r "isActive" src/`

**Step 2: Remove isActive prop from all usages**

For each file found, remove the `isActive` prop:

```typescript
// BEFORE:
<SessionCard
  session={session}
  isSelected={selectedId === session.id}
  isActive={session.modifiedAt > fiveMinutesAgo}
  onClick={() => handleClick(session)}
/>

// AFTER:
<SessionCard
  session={session}
  isSelected={selectedId === session.id}
  onClick={() => handleClick(session)}
/>
```

**Step 3: Verify build**

Run: `npm run build`
Expected: No type errors

**Step 4: Commit**

```bash
git add .
git commit -m "chore: remove isActive prop from SessionCard usages

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Task 11: Final Integration Test

**Step 1: Start dev server**

Run: `npm run dev`

**Step 2: Verify all fixes**

Checklist:
- [ ] SessionCard shows: preview, lastMessage, files, tool counts, message count, turns, skills
- [ ] No "Active" indicators anywhere
- [ ] Sidebar tool stats show real numbers
- [ ] StatsDashboard skills show real numbers
- [ ] Long conversations scroll smoothly (virtualization working)
- [ ] XML content renders as collapsible cards

**Step 3: Run build**

Run: `npm run build`
Expected: Clean build with no errors

**Step 4: Commit any final fixes**

```bash
git add .
git commit -m "chore: final integration verification

Co-Authored-By: Claude Opus 4.5 <noreply@anthropic.com>"
```

---

## Summary

| Task | Description | Files |
|------|-------------|-------|
| 1 | Debug data pipeline | sessions.ts |
| 2 | Fix tool count extraction | sessions.ts |
| 3 | Add messageCount/turnCount | sessions.ts, use-projects.ts |
| 4 | Remove active indicators | SessionCard, Sidebar, StatsDashboard |
| 5 | Enhance SessionCard display | SessionCard.tsx |
| 6 | Install react-virtuoso | package.json |
| 7 | Virtualize ConversationView | ConversationView.tsx |
| 8 | Create XmlCard component | XmlCard.tsx |
| 9 | Integrate XmlCard | Message.tsx |
| 10 | Update SessionCard usages | Various |
| 11 | Final integration test | All |

Total: 11 tasks with atomic commits for each change.
