---
status: draft
date: 2026-01-28
---

# Session View UX Polish — 6 Issues Design

## Context

The session detail view (`/session/:project/:id`) has 6 UX issues that collectively
degrade the reading and sharing experience. This design addresses all 6 with root-cause
analysis and targeted fixes.

## Issues & Root Causes

### Issue 1: Empty messages showing nothing

**Root cause:** Claude's extended thinking produces JSONL lines where
`message.content = [{"type":"thinking","thinking":"..."}]`. The `ContentBlock` enum
maps `thinking` to `#[serde(other)] Other`. `extract_assistant_content` skips `Other`
blocks, returning empty string. Unlike user messages (which have an empty guard at
parser.rs:96), assistant messages push unconditionally at line 118.

**Data:** 473 thinking blocks across 50 sessions. Always standalone (never combined with
text in the same message). Always immediately precede the actual response.

### Issue 2: "Show X more lines" flashes and collapses

**Root cause:** `Virtuoso` virtualizes the list — unmounting/remounting items as they
scroll. When `CodeBlock` expands (height change), Virtuoso recalculates layout and may
remount the component. `useState(false)` resets to collapsed.

### Issue 3: Text blocks collapsed at 5 lines

**Current behavior:** `COLLAPSE_THRESHOLD = 5` applies to ALL code blocks including
plain text rendered as code. Text should flow naturally; only large code files need
collapsing.

### Issue 4: No per-message copy button

**Current:** Only `CodeBlock` has a copy button. No way to copy an entire message.

### Issue 5: Terminal backslash-newlines in user input

**Current:** User messages from terminal contain literal `\` + newline sequences.
Rendered as-is, they look like raw terminal input.

### Issue 6: User identity ("U" / "You")

**Current:** Blue box with "U" letter, label "You". Feels personal/informal for a
tool meant for sharing sessions for education, showcase, and demo purposes.

## Design

### D1: Thinking Blocks — Merged with Response

**Backend (`crates/core/src/types.rs`):**

Add `Thinking` variant to `ContentBlock`:

```rust
pub enum ContentBlock {
    Text { text: String },
    Thinking { thinking: String },  // NEW
    ToolUse { name: String, input: Option<Value> },
    ToolResult { content: Option<Value> },
    #[serde(other)]
    Other,
}
```

**Backend (`crates/core/src/parser.rs`):**

- `extract_assistant_content` returns thinking text alongside content text
- Add `thinking: Option<String>` field to `Message` struct
- **Merge logic:** When an assistant message has ONLY thinking (no text, no tools),
  don't emit it as a separate message. Instead, store the thinking text and attach it
  to the NEXT assistant message as its `thinking` field.
- Add empty guard: if content, tools, AND thinking are all empty, skip the message

**API response:**

```json
{
  "role": "assistant",
  "content": "Here's what I found...",
  "thinking": "Let me analyze this step by step...",
  "toolCalls": [...]
}
```

**Frontend (`Message.tsx`):**

New `ThinkingBlock` inline component rendered above the message content:

- Default **collapsed**: shows a single line with brain icon + "Thinking..." + dimmed
  preview of first ~80 chars
- Expanded: shows full thinking text as plain paragraphs
- Styling: light purple/indigo tint background, text slightly dimmed, italic
- Toggle on click anywhere on the collapsed bar
- No avatar or header — it's visually part of the response bubble, not a separate message

### D2: Expand/Collapse State Survives Virtuoso

**Approach:** React Context at `ConversationView` level.

Create `ExpandContext` that holds a `Set<string>` of expanded block IDs.
Each `CodeBlock` gets a stable ID derived from `messageIndex-blockIndex`.
State lives above Virtuoso, survives unmount/remount cycles.

```tsx
// ConversationView.tsx
const [expandedBlocks, setExpandedBlocks] = useState<Set<string>>(new Set())
const toggleBlock = (id: string) => { ... }

<ExpandContext.Provider value={{ expandedBlocks, toggleBlock }}>
  <Virtuoso ... />
</ExpandContext.Provider>
```

```tsx
// CodeBlock.tsx
const { expandedBlocks, toggleBlock } = useContext(ExpandContext)
const isExpanded = expandedBlocks.has(blockId)
```

### D3: Text Never Collapses, Code at 25+ Lines

- Remove collapse behavior from text blocks entirely
- Change `COLLAPSE_THRESHOLD` from 5 to 25
- ReactMarkdown text segments render fully (no truncation)
- Only fenced code blocks (``` language) collapse at 25+ lines

Detection: `CodeBlock` already only renders for fenced code blocks (the `code` component
in ReactMarkdown checks for `language-*` className). Text content flows through `<p>`,
`<ul>`, etc. — never hits `CodeBlock`. So this is simply changing the threshold constant.

### D4: Per-Message Copy Button

Add a copy button to the message header (next to timestamp).

- Copies **markdown source** of `message.content` (preserves formatting for
  developer audience pasting into GitHub, Notion, Slack)
- Small clipboard icon, appears on hover (desktop) or always visible (mobile)
- Same copy feedback pattern as CodeBlock: icon changes to checkmark for 2s
- Position: right side of header row, before timestamp

```tsx
// In Message header
<div className="flex items-center gap-2">
  <button onClick={handleCopyMessage} className="opacity-0 group-hover:opacity-100 ...">
    {copied ? <Check /> : <Copy />}
  </button>
  {time && <span className="text-xs text-gray-400">{time}</span>}
</div>
```

### D5: Clean Terminal Backslash-Newlines

**Backend (`crates/core/src/parser.rs`):**

In user message processing, after `clean_command_tags`, add a step to normalize
terminal line continuations:

```rust
// Replace backslash + newline with actual newline
let content = content.replace("\\\n", "\n");
// Collapse multiple blank lines into max 2
let content = collapse_blank_lines(&content);
```

This transforms raw terminal input into clean paragraphs before it reaches the frontend.
ReactMarkdown then renders them as proper paragraphs.

### D6: "Human" Identity with Person Icon

Replace user identity across the app:

| Before | After |
|--------|-------|
| Blue "U" square | Person silhouette icon (`User` from lucide-react) on subtle gray bg |
| "You" label | "Human" label |

**Frontend (`Message.tsx`):**

```tsx
// Avatar
{isUser ? (
  <div className="w-8 h-8 rounded flex items-center justify-center bg-gray-200 text-gray-600">
    <User className="w-4 h-4" />
  </div>
) : (
  <div className="w-8 h-8 rounded flex items-center justify-center bg-orange-500 text-white">
    C
  </div>
)}

// Name
<span className="font-medium text-gray-900">
  {isUser ? 'Human' : 'Claude'}
</span>
```

### D7: Fix Command Tag Parsing (discovered during investigation)

**Root cause:** `clean_command_tags` strips `<command-args>` content (the actual user
prompt) instead of extracting it. Also misses `<command-message>` tag.

**Fix in `parser.rs`:**

```rust
fn clean_command_tags(content: &str) -> String {
    // 1. Extract content from <command-args>...</command-args> if present
    //    (this IS the user's actual prompt text)
    // 2. Strip <command-message>...</command-message> (metadata only)
    // 3. Strip <command-name>...</command-name> (metadata only)
    // 4. If <command-args> was found, use its inner content
    //    Otherwise, use remaining content after stripping other tags
}
```

Use `(?s)` dotall flag for the `<command-args>` regex so it matches multiline content
including `<` characters:

```rust
let args_extract = Regex::new(r"(?s)<command-args>(.*?)</command-args>").unwrap();
let msg_strip = Regex::new(r"(?s)<command-message>.*?</command-message>\s*").unwrap();
let name_strip = Regex::new(r"(?s)<command-name>.*?</command-name>\s*").unwrap();
```

## Files to Change

### Backend (Rust)

| File | Changes |
|------|---------|
| `crates/core/src/types.rs` | Add `Thinking` variant to `ContentBlock`, add `thinking` field to `Message` |
| `crates/core/src/parser.rs` | Merge thinking blocks, fix command tags, normalize backslash-newlines, add assistant empty guard |

### Frontend (React)

| File | Changes |
|------|---------|
| `src/components/Message.tsx` | ThinkingBlock, copy button, Human identity, group-hover |
| `src/components/CodeBlock.tsx` | Use ExpandContext, threshold 5→25 |
| `src/components/ConversationView.tsx` | Provide ExpandContext |
| `src/components/ThinkingBlock.tsx` | New: collapsible thinking display |

### New Files

| File | Purpose |
|------|---------|
| `src/components/ThinkingBlock.tsx` | Collapsible thinking block UI |
| `src/contexts/ExpandContext.tsx` | Expand state context for Virtuoso |

## Implementation Order

1. **D7** — Fix command tag parsing (unblocks correct first-message display)
2. **D1** — Thinking blocks (biggest visible improvement, backend + frontend)
3. **D2** — Expand/collapse context (fixes the bug)
4. **D3** — Threshold change (one-line fix after D2)
5. **D5** — Backslash normalization (backend one-liner)
6. **D6** — Human identity (frontend one-liner)
7. **D4** — Per-message copy button (frontend, no backend dependency)

## Out of Scope

- Image content blocks (5 found, base64 PNG — future enhancement)
- Export updates (HTML/PDF export should reflect new rendering — follow-up)
- Thinking block token cost display (available in JSONL usage data — future)
