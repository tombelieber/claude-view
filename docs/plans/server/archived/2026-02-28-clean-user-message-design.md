# Clean User Message + IDE File Chip

> **Date:** 2026-02-28
> **Status:** Done (implemented 2026-03-01)
> **Scope:** Rust live_parser, LiveSession wire format, web SessionCard, mobile SessionCard

---

## Problem

`lastUserMessage` is a 200-char truncation of raw JSONL user content. It includes XML tags injected by Claude Code's hook system (`<system-reminder>`, `<ide_opened_file>`, `<command-name>`, etc.) that are not user-typed text. These tags:

1. Pollute the session card preview with noise
2. Eat the 200-char truncation budget, leaving little or no actual user text
3. Contain useful context (`<ide_opened_file>`) that deserves dedicated UI

## Decision

- **Strip noise tags in Rust before truncation** so the 200-char budget goes to real user text
- **Extract IDE file context** from `<ide_opened_file>` as a structured field
- **Surface the file as a chip** on both web and mobile session cards
- **Additive only** — no existing layout changes, chip only renders when data exists

## Tags Classification

| Tag | Action | Reason |
|-----|--------|--------|
| `<system-reminder>...</system-reminder>` | Strip | Hook plumbing noise |
| `<ide_opened_file>...</ide_opened_file>` | Extract filename, strip tag | Useful context for chip |
| `<ide_selection>...</ide_selection>` | Strip (not surfaced for now) | Too long for card |
| `<command-name>...</command-name>` | Strip | Internal slash command metadata |
| `<command-args>...</command-args>` | Strip | Internal slash command metadata |
| `<command-message>...</command-message>` | Strip | Internal slash command metadata |
| `<local-command-stdout>...</local-command-stdout>` | Strip | Command output noise |
| `<local-command-caveat>...</local-command-caveat>` | Strip | Command caveat noise |
| `<task-notification>...</task-notification>` | Strip | Already tracked via subAgents |
| `<user-prompt-submit-hook>...</user-prompt-submit-hook>` | Strip | Hook metadata |

## Data Flow

```
JSONL raw content (with tags)
  -> live_parser.rs: strip_noise_tags() + extract_ide_file()
    -> truncate_str(clean_text, 200)
      -> LiveLine { content_preview: "fix the bug", ide_file: Some("auth.rs") }
        -> accumulator -> LiveSession { lastUserMessage, lastUserFile }
          -> relay -> RelaySession { lastUserMessage, lastUserFile }
            -> SessionCard (web + mobile)
```

## Wire Format Change

```diff
// LiveSession (Rust) + RelaySession (TS)
  lastUserMessage: string        // now clean, no tags
+ lastUserFile: string | null    // "auth.rs" extracted from <ide_opened_file>
```

Additive field — no breaking change. Old clients ignore unknown fields.

## Rust Changes

### `crates/core/src/live_parser.rs`

New function `strip_noise_tags(content: &str) -> (String, Option<String>)`:
- Returns `(clean_text, ide_file)`
- Strips all noise tags listed above using byte-level or regex matching
- Extracts last path component from `<ide_opened_file>The user opened the file /full/path/to/auth.rs in the IDE...</ide_opened_file>`
- Called in `extract_content_and_tools()` before `truncate_str()`

New field on `LiveLine`: `ide_file: Option<String>`

### `crates/core/src/accumulator.rs`

Track `last_user_file: String` alongside `last_user_message`. Updated when a non-meta user line has `ide_file.is_some()`.

### `crates/server/src/live/state.rs`

Add `last_user_file: Option<String>` to `LiveSession` struct.

## Frontend Changes

### Web (`apps/web/src/components/live/SessionCard.tsx`)

Add optional file chip between title/message and spinner row:

```tsx
{session.lastUserFile && (
  <p className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500 mb-1">
    <FileText className="h-3 w-3" />
    <span className="font-mono">{session.lastUserFile}</span>
  </p>
)}
```

`cleanPreviewText()` remains as a safety net — belt + suspenders.

### Web types (`apps/web/src/components/live/use-live-sessions.ts`)

Add `lastUserFile?: string | null` to `LiveSession` interface.

### Mobile (`apps/mobile/components/SessionCard.tsx`)

Add file chip row and clean the message text:

```tsx
{session.lastUserFile && (
  <XStack items="center" gap="$1" mt="$1">
    <FileText size={12} color="$gray400" />
    <Text color="$gray400" fontSize="$xs" fontFamily="$mono">
      {session.lastUserFile}
    </Text>
  </XStack>
)}
```

### Relay types (`packages/shared/src/types/relay.ts`)

Add `lastUserFile: string | null` to `RelaySession` interface.

## What's NOT in scope

- `<ide_selection>` extraction — stripped but not surfaced (revisit later)
- Slash command display — stripped silently
- Web frontend full parser changes — it already strips tags via `cleanPreviewText()`
- ListView table changes — only card views get the chip
