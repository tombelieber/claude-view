# Clean User Message + IDE File Chip — Implementation Plan

> **Status:** DONE (2026-03-01) — all 7 tasks implemented, shippable audit passed (SHIP IT)
>
> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Strip XML noise tags from `lastUserMessage` before truncation so session cards show real user text, and surface IDE file context as a chip.

**Architecture:** Rust `live_parser.rs` strips tags and extracts IDE filename before the 200-char truncation. New `ide_file` field flows through `LiveLine` → accumulator → `LiveSession` → relay → TS types → both web and mobile SessionCard components.

**Tech Stack:** Rust (live_parser, accumulator, state), TypeScript (web React + mobile Expo/Tamagui), relay protocol

**Design doc:** `docs/plans/2026-02-28-clean-user-message-design.md`

## Completion Summary

| Task | Commit | Description |
|------|--------|-------------|
| 1 | `39edccc8` | Rust `strip_noise_tags()` + 11 tests |
| 2 | `46969fe3` | Wire into live parser, add `ide_file` to `LiveLine` |
| 3 | `18ffa6ba` | Thread `last_user_file` through accumulator → LiveSession → manager |
| 4 | `fd6b0675` | Update TS types (generated + relay + web) |
| 5 | `6bff3e60` | Web SessionCard IDE file chip |
| 6 | `2158a9c7` | Mobile SessionCard + DetailSheet file chip |
| 7 | — | End-to-end wiring verified (10/10 layers) |

**Shippable audit:** Plan Compliance 82/82, Wiring 10/10, 0 blockers, core 652 + server 525 tests pass.

---

### Task 1: Rust — `strip_noise_tags()` function + tests

**Files:**
- Modify: `crates/core/src/live_parser.rs`

**Step 1: Write failing tests for tag stripping**

Add at the bottom of the `#[cfg(test)] mod tests` block in `crates/core/src/live_parser.rs`:

```rust
#[test]
fn test_strip_noise_tags_system_reminder() {
    let input = "<system-reminder>SessionStart:startup hook success: Success</system-reminder>fix the bug";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "fix the bug");
    assert!(file.is_none());
}

#[test]
fn test_strip_noise_tags_ide_opened_file() {
    let input = "<ide_opened_file>The user opened the file /Users/me/project/src/auth.rs in the IDE. This may or may not be related to the current task.</ide_opened_file> continue";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "continue");
    assert_eq!(file.as_deref(), Some("auth.rs"));
}

#[test]
fn test_strip_noise_tags_multiple_tags() {
    let input = "<system-reminder>hook data</system-reminder><ide_opened_file>The user opened the file /path/to/main.rs in the IDE.</ide_opened_file><ide_selection>some code</ide_selection> do the thing";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "do the thing");
    assert_eq!(file.as_deref(), Some("main.rs"));
}

#[test]
fn test_strip_noise_tags_no_tags() {
    let input = "just a normal message";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "just a normal message");
    assert!(file.is_none());
}

#[test]
fn test_strip_noise_tags_only_tags() {
    let input = "<system-reminder>hook stuff</system-reminder>";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "");
    assert!(file.is_none());
}

#[test]
fn test_strip_noise_tags_command_tags() {
    let input = "<command-name>/clear</command-name><command-message>Clearing context</command-message> hello";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "hello");
    assert!(file.is_none());
}

#[test]
fn test_strip_noise_tags_nested_path() {
    let input = "<ide_opened_file>The user opened the file /deep/nested/path/to/component.tsx in the IDE.</ide_opened_file>review this";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "review this");
    assert_eq!(file.as_deref(), Some("component.tsx"));
}

#[test]
fn test_strip_noise_tags_user_prompt_submit_hook() {
    let input = "<user-prompt-submit-hook>hook output</user-prompt-submit-hook>fix tests";
    let (clean, file) = strip_noise_tags(input);
    assert_eq!(clean, "fix tests");
    assert!(file.is_none());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core strip_noise_tags -- --nocapture`
Expected: FAIL — `strip_noise_tags` function does not exist yet.

**Step 3: Implement `strip_noise_tags()`**

Add this function above `extract_content_and_tools()` in `crates/core/src/live_parser.rs` (around line 764):

```rust
/// Strip XML noise tags from user message content and extract IDE file context.
///
/// Returns `(clean_text, ide_file)` where:
/// - `clean_text` is the content with all noise tags removed, trimmed
/// - `ide_file` is the last path component from `<ide_opened_file>` if present
///
/// Tags stripped: system-reminder, ide_opened_file, ide_selection, command-name,
/// command-args, command-message, local-command-stdout, local-command-caveat,
/// task-notification, user-prompt-submit-hook.
///
/// NOTE: The IDE filename extraction regex ("the file\s+(\S+)\s+in the IDE") is
/// coupled to Claude Code's current hook output format. If the format changes,
/// extraction gracefully degrades to `None`.
fn strip_noise_tags(content: &str) -> (String, Option<String>) {
    use regex_lite::Regex;
    use std::sync::OnceLock;

    // `regex-lite` does NOT support backreferences (\1), so each tag must be
    // enumerated with its explicit closing tag. Uses OnceLock to match codebase
    // convention (see cli.rs, sync.rs, metrics.rs).
    static NOISE_TAGS: OnceLock<Regex> = OnceLock::new();
    let noise_re = NOISE_TAGS.get_or_init(|| {
        Regex::new(concat!(
            r"(?s)<system-reminder>.*?</system-reminder>",
            r"|<ide_selection>.*?</ide_selection>",
            r"|<command-name>.*?</command-name>",
            r"|<command-args>.*?</command-args>",
            r"|<command-message>.*?</command-message>",
            r"|<local-command-stdout>.*?</local-command-stdout>",
            r"|<local-command-caveat>.*?</local-command-caveat>",
            r"|<task-notification>.*?</task-notification>",
            r"|<user-prompt-submit-hook>.*?</user-prompt-submit-hook>",
        )).unwrap()
    });

    // Separate regex for ide_opened_file to extract filename before stripping
    static IDE_FILE_TAG: OnceLock<Regex> = OnceLock::new();
    let ide_tag_re = IDE_FILE_TAG.get_or_init(|| {
        Regex::new(r"(?s)<ide_opened_file>.*?</ide_opened_file>").unwrap()
    });

    static IDE_FILE_PATH: OnceLock<Regex> = OnceLock::new();
    let ide_path_re = IDE_FILE_PATH.get_or_init(|| {
        Regex::new(r"the file\s+(\S+)\s+in the IDE").unwrap()
    });

    // Extract IDE file before stripping
    let ide_file = ide_tag_re.find(content).and_then(|m| {
        ide_path_re.captures(m.as_str()).and_then(|caps| {
            caps.get(1).map(|p| {
                let path = p.as_str();
                // Extract last path component (filename)
                path.rsplit('/').next().unwrap_or(path).to_string()
            })
        })
    });

    // Strip all noise tags
    let cleaned = noise_re.replace_all(content, "");
    // Strip ide_opened_file tag
    let cleaned = ide_tag_re.replace_all(&cleaned, "");
    let cleaned = cleaned.trim().to_string();

    (cleaned, ide_file)
}
```

**Note:** `regex-lite` is already a workspace dependency in `crates/core/Cargo.toml` — no `Cargo.toml` change needed. Uses `OnceLock` (not `LazyLock`) to match existing codebase convention. The `OnceLock<Regex>` pattern is new (existing regex uses in `parser.rs`/`discovery.rs` recompile each call), but correct for a per-line hot path.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core strip_noise_tags -- --nocapture`
Expected: All 8 tests PASS.

**Step 5: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "feat(core): add strip_noise_tags() for clean user message extraction"
```

---

### Task 2: Rust — Wire `strip_noise_tags` into `extract_content_and_tools` + add `ide_file` to `LiveLine`

**Files:**
- Modify: `crates/core/src/live_parser.rs` (LiveLine struct + extract_content_and_tools fn)

**Step 1: Write failing test for content_preview stripping**

Add to tests in `crates/core/src/live_parser.rs`:

```rust
#[test]
fn test_parse_tail_strips_noise_tags_from_preview() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tags.jsonl");
    let mut f = File::create(&path).unwrap();
    writeln!(
        f,
        r#"{{"type":"user","message":{{"role":"user","content":"<system-reminder>hook data</system-reminder>fix the bug"}}}}"#
    ).unwrap();
    f.flush().unwrap();

    let finders = TailFinders::new();
    let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content_preview, "fix the bug");
    assert!(lines[0].ide_file.is_none());
}

#[test]
fn test_parse_tail_extracts_ide_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ide.jsonl");
    let mut f = File::create(&path).unwrap();
    writeln!(
        f,
        r#"{{"type":"user","message":{{"role":"user","content":"<ide_opened_file>The user opened the file /src/auth.rs in the IDE.</ide_opened_file> continue"}}}}"#
    ).unwrap();
    f.flush().unwrap();

    let finders = TailFinders::new();
    let (lines, _) = parse_tail(&path, 0, &finders).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].content_preview, "continue");
    assert_eq!(lines[0].ide_file.as_deref(), Some("auth.rs"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core parse_tail_strips_noise -- --nocapture && cargo test -p claude-view-core parse_tail_extracts_ide -- --nocapture`
Expected: FAIL — `ide_file` field doesn't exist on `LiveLine`.

> **Note:** `parse_tail` returns `(Vec<LiveLine>, u64)` (not `usize`). The tests discard the second value with `_`, so this is correct.

**Step 3: Add `ide_file` to `LiveLine` and wire stripping**

In `crates/core/src/live_parser.rs`:

a) Add field to `LiveLine` struct (after `has_system_prefix`, around line 93):
```rust
    /// Filename extracted from `<ide_opened_file>` tag, if present.
    pub ide_file: Option<String>,
```

b) Add `ide_file: None` to ALL `LiveLine` construction sites:

- **Error fallback** (line ~288, the `Err(_)` branch of `serde_json::from_slice`): Add `ide_file: None,` after `skill_names: Vec::new(),` (line ~315) in the `LiveLine { ... }` block.
- **Main return** (line ~733): Add `ide_file,` after `has_system_prefix,` in the final `LiveLine { ... }` block.

c) In `extract_content_and_tools()`, change the return type to include `ide_file`:
```rust
fn extract_content_and_tools(
    parsed: &serde_json::Value,
    _finders: &TailFinders,
) -> (String, Vec<String>, Vec<String>, bool, Option<String>) {
```

d) Add `let mut ide_file: Option<String> = None;` at the top of `extract_content_and_tools()`, alongside the existing `preview`, `tool_names`, etc. declarations.

e) In the `Some(serde_json::Value::String(s))` arm (line ~779), apply stripping:
```rust
Some(serde_json::Value::String(s)) => {
    let (stripped, file) = strip_noise_tags(s);
    preview = truncate_str(&stripped, 200);
    ide_file = file;
}
```

f) In the `Some(serde_json::Value::Array(blocks))` arm, for `"text"` blocks (line ~786):
```rust
Some("text") => {
    if preview.is_empty() {
        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
            let (stripped, file) = strip_noise_tags(text);
            preview = truncate_str(&stripped, 200);
            if ide_file.is_none() {
                ide_file = file;
            }
        }
    }
}
```

g) Return `ide_file` as the 5th tuple element at the end of the function.

h) Update the call site (~line 339) to destructure the 5th element:
```rust
let (content_preview, tool_names, skill_names, is_tool_result, ide_file) =
    extract_content_and_tools(content_source, finders);
```

i) Wire `ide_file` into the final `LiveLine` return (around line 733):
```rust
ide_file,
```

**IMPORTANT: `has_system_prefix` interaction.** The existing `has_system_prefix` detection (line ~345) runs AFTER `extract_content_and_tools()` and checks `content_preview.starts_with(...)` for tags like `<command-name>/clear`. After our stripping, `content_preview` will no longer start with those tags, so `has_system_prefix` would incorrectly become `false`.

**Fix:** Move the `has_system_prefix` detection to run on the RAW content BEFORE stripping. In the call site block (~line 339), after destructuring `extract_content_and_tools`, the `has_system_prefix` check at line ~345 already reads from `content_preview`. We must change it to read from the raw source instead:

```rust
// Detect system-injected prefixes from RAW content (before stripping).
// NOTE: .as_str() returns None for array content, defaulting to "" (false).
// This is safe because system-prefix messages (<command-name>, <task-notification>,
// <local-command-caveat>, <local-command-stdout>) always use string content in
// Claude Code JSONL — never inside content arrays.
let has_system_prefix = if line_type == LineType::User {
    let raw = content_source
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("");
    let c = raw.trim_start();
    c.starts_with("<local-command-caveat>")
        || c.starts_with("<local-command-stdout>")
        || c.starts_with("<command-name>/clear")
        || c.starts_with("<command-name>/context")
        || c.starts_with("This session is being continued")
        || c.starts_with("<task-notification>")
} else {
    false
};
```

This preserves the existing behavior: slash commands and system-injected lines still get `has_system_prefix = true`, which gates `current_turn_started_at` in `manager.rs:1114`.

**Step 4: Run tests**

Run: `cargo test -p claude-view-core parse_tail_strips_noise parse_tail_extracts_ide -- --nocapture`
Expected: PASS.

Also run existing live_parser tests to check for regressions:
Run: `cargo test -p claude-view-core live_parser -- --nocapture`
Expected: All existing tests still pass.

**Step 5: Commit**

```bash
git add crates/core/src/live_parser.rs
git commit -m "feat(core): wire strip_noise_tags into live parser, add ide_file to LiveLine"
```

---

### Task 3: Rust — Wire `ide_file` through accumulator → LiveSession → manager

**Files:**
- Modify: `crates/core/src/accumulator.rs` (SessionAccumulator + RichSessionData)
- Modify: `crates/server/src/live/state.rs` (LiveSession struct)
- Modify: `crates/server/src/live/manager.rs` (JsonlMetadata + apply fn + both accumulation loops)
- Modify: `crates/server/src/routes/hooks.rs` (3 LiveSession construction sites: lines ~125, ~204, ~873)
- Modify: `crates/server/src/routes/terminal.rs` (1 LiveSession construction in test: line ~1182)
- Modify: `crates/server/src/routes/sessions.rs` (1 LiveSession construction in test: line ~1621)

**Step 1: Add `last_user_file` to accumulator**

In `crates/core/src/accumulator.rs`:

a) Add field to `SessionAccumulator` (after `last_user_message`, line ~25):
```rust
    pub last_user_file: Option<String>,
```

b) Initialize in `new()` (after `last_user_message: String::new()`, line ~63):
```rust
    last_user_file: None,
```

c) In `process_line()` where `last_user_message` is set (around line 165), add:
```rust
    self.last_user_message = line.content_preview.clone();
    if line.ide_file.is_some() {
        self.last_user_file = line.ide_file.clone();
    }
```

d) Add to `RichSessionData` (after `last_user_message`, line ~50):
```rust
    pub last_user_file: Option<String>,
```

e) Wire in `finish()` (after the `last_user_message` block, around line ~416):
```rust
    last_user_file: self.last_user_file.clone(),
```

**Step 2: Add `last_user_file` to `LiveSession` in state.rs**

In `crates/server/src/live/state.rs`, add after `last_user_message` (line ~105):
```rust
    /// Filename from `<ide_opened_file>` tag in the last user message, if present.
    pub last_user_file: Option<String>,
```

**Step 3: Wire through manager.rs**

In `crates/server/src/live/manager.rs`:

a) Add to `SessionAccumulator` struct (after `last_user_message`, line ~52):
```rust
    last_user_file: Option<String>,
```

b) Initialize in `SessionAccumulator` default (after `last_user_message: String::new()`, line ~98):
```rust
    last_user_file: None,
```

c) Add to `JsonlMetadata` struct (after `last_user_message`, line ~126):
```rust
    last_user_file: Option<String>,
```

d) In `build_recovered_session()` (after `last_user_message: String::new()`, line ~171):
```rust
    last_user_file: None,
```

e) In `apply_jsonl_metadata()` (after the `last_user_message` block, around line ~299):
```rust
    if m.last_user_file.is_some() {
        session.last_user_file = m.last_user_file.clone();
    }
```

f) In the live accumulation loop where `acc.last_user_message` is set (~line 1104):
```rust
    acc.last_user_message = line.content_preview.clone();
    if line.ide_file.is_some() {
        acc.last_user_file = line.ide_file.clone();
    }
```

g) Where `JsonlMetadata` is built from the live accumulator (~line 553):
```rust
    last_user_file: acc.last_user_file.clone(),
```

h) Where `JsonlMetadata` is built from the batch accumulator (the `finish()` path, ~line 1382):
```rust
    last_user_file: acc.last_user_file.clone(),
```

i) Add `last_user_file: None,` to ALL other `LiveSession` construction sites (7 total exist; `build_recovered_session` is covered above, the struct definition in `state.rs` is covered in Step 2). The remaining 5:

In `crates/server/src/routes/hooks.rs`, add `last_user_file: None,` to the `LiveSession { ... }` blocks at lines ~125, ~204, and ~873.

In `crates/server/src/routes/terminal.rs`, add `last_user_file: None,` to the `LiveSession { ... }` block at line ~1182.

In `crates/server/src/routes/sessions.rs`, add `last_user_file: None,` to the `LiveSession { ... }` block at line ~1621.

**Step 4: Run Rust tests**

Run: `cargo test -p claude-view-core accumulator -- --nocapture`
Run: `cargo test -p claude-view-server -- --nocapture`
Expected: All pass (new field is `Option`, defaults to `None`).

**Step 5: Commit**

```bash
git add crates/core/src/accumulator.rs crates/server/src/live/state.rs crates/server/src/live/manager.rs crates/server/src/routes/hooks.rs crates/server/src/routes/terminal.rs crates/server/src/routes/sessions.rs
git commit -m "feat: wire last_user_file through accumulator → LiveSession → manager"
```

---

### Task 4: TypeScript — Update types (generated + relay + web)

**Files:**
- Regenerate: `packages/shared/src/types/generated/LiveSession.ts` (auto-generated by ts-rs — **DO NOT edit manually**)
- Modify: `packages/shared/src/types/relay.ts`
- Modify: `apps/web/src/components/live/use-live-sessions.ts`

**Step 1: Regenerate TS types from Rust**

`LiveSession.ts` is auto-generated — the file header says "Do not edit this file manually." After adding `last_user_file: Option<String>` to the Rust `LiveSession` struct (Task 3 Step 2), regenerate:

Run: `./scripts/generate-types.sh`
Expected: `packages/shared/src/types/generated/LiveSession.ts` now contains `lastUserFile: string | null`.

Verify: `grep lastUserFile packages/shared/src/types/generated/LiveSession.ts`
Expected: The field appears automatically. If `generate-types.sh` fails, ONLY THEN manually add the field as a fallback.

**Step 2: Add to `RelaySession`**

In `packages/shared/src/types/relay.ts`, add after `lastUserMessage` (line ~94):
```typescript
  lastUserFile: string | null
```

**Step 3: Add to web `LiveSession` interface**

In `apps/web/src/components/live/use-live-sessions.ts`, add after `lastUserMessage` (line ~20):
```typescript
  lastUserFile?: string | null
```

**Step 4: Verify TypeScript compiles**

Run: `cd apps/web && npx tsc --noEmit`
Run: `cd packages/shared && npx tsc --noEmit`
Expected: No errors.

**Step 5: Commit**

```bash
git add packages/shared/src/types/generated/LiveSession.ts packages/shared/src/types/relay.ts apps/web/src/components/live/use-live-sessions.ts
git commit -m "feat(types): add lastUserFile to LiveSession, RelaySession, web types"
```

---

### Task 5: Web — Add file chip to SessionCard

**Files:**
- Modify: `apps/web/src/components/live/SessionCard.tsx`

**Step 1: Add import**

Add `FileText` to the lucide-react import at `apps/web/src/components/live/SessionCard.tsx:1`:

```typescript
import {
  AlertTriangle,
  Archive,
  // ... existing imports ...
  FileText,    // <-- add this
  GitBranch,
  // ...
} from 'lucide-react'
```

**Step 2: Add file chip after last message / title**

In `SessionCard.tsx`, after the `showLastMsg` block (after line ~165), add:

```tsx
      {/* IDE file context chip */}
      {session.lastUserFile && (
        <p className="flex items-center gap-1 text-xs text-gray-400 dark:text-gray-500 mb-1">
          <FileText className="h-3 w-3 flex-shrink-0" />
          <span className="font-mono truncate">{session.lastUserFile}</span>
        </p>
      )}
```

**Step 3: Build and verify**

Run: `cd apps/web && bun run build`
Expected: Builds without errors.

**Step 4: Commit**

```bash
git add apps/web/src/components/live/SessionCard.tsx
git commit -m "feat(web): add IDE file chip to live SessionCard"
```

---

### Task 6: Mobile — Add file chip to SessionCard + clean message text

**Files:**
- Modify: `apps/mobile/components/SessionCard.tsx`
- Modify: `apps/mobile/components/SessionDetailSheet.tsx`

**Step 1: No new deps needed**

Use `@expo/vector-icons` (already in `apps/mobile/package.json`) instead of `lucide-react-native`. Rationale: `@expo/vector-icons` is Expo's blessed icon solution — prebuilt into Expo Go, uses bitmap fonts (not SVG), zero native modules, zero additional build steps. Adding `lucide-react-native` + `react-native-svg` would introduce a native binary dependency (~200KB+) for a single 12px icon.

**Step 2: Update SessionCard**

In `apps/mobile/components/SessionCard.tsx`:

a) Add import for the icon:
```typescript
import { Ionicons } from '@expo/vector-icons'
```

b) Add the lastUserMessage display + file chip. The mobile SessionCard currently renders only: project name, status+model row, cost+tokens row — it does NOT display `lastUserMessage` today. We add both the message and the file chip as new rows. After the status XStack closing tag (line ~40, the `</XStack>` that closes the status/model row), before the cost XStack at line ~42, add:

```tsx
        {/* Last user message */}
        {session.lastUserMessage ? (
          <Text color="$gray200" fontSize="$sm" numberOfLines={2} mt="$2">
            {session.lastUserMessage}
          </Text>
        ) : null}
        {/* IDE file chip */}
        {session.lastUserFile ? (
          <XStack items="center" gap="$1" mt="$1">
            <Ionicons name="document-text-outline" size={12} color="#9ca3af" />
            <Text color="$gray400" fontSize="$xs" fontFamily="$mono">
              {session.lastUserFile}
            </Text>
          </XStack>
        ) : null}
```

> **Note:** `Ionicons` `color` prop takes a CSS color string, not a Tamagui token. Use `"#9ca3af"` (gray-400 equivalent) to match surrounding Tamagui `$gray400` text.

**Step 3: Update SessionDetailSheet**

In `apps/mobile/components/SessionDetailSheet.tsx`, update the "Last Activity" section (line ~58, the `{session.lastUserMessage ? (` block) to also show the file chip:

```tsx
          {session.lastUserMessage ? (
            <>
              <SectionLabel>Last Activity</SectionLabel>
              <Text color="$gray200" fontSize="$sm" numberOfLines={4}>
                {session.lastUserMessage}
              </Text>
              {session.lastUserFile ? (
                <XStack items="center" gap="$1" mt="$2">
                  <Ionicons name="document-text-outline" size={14} color="#9ca3af" />
                  <Text color="$gray400" fontSize="$xs" fontFamily="$mono">
                    Viewing: {session.lastUserFile}
                  </Text>
                </XStack>
              ) : null}
            </>
          ) : null}
```

Add the import at top (note: `SectionLabel` is a local component defined at lines 86–98, NOT an import):
```typescript
import { Ionicons } from '@expo/vector-icons'
```

**Step 4: Verify mobile builds**

Run: `cd apps/mobile && npx tsc --noEmit`
Expected: No type errors.

**Step 5: Commit**

```bash
git add apps/mobile/components/SessionCard.tsx apps/mobile/components/SessionDetailSheet.tsx
git commit -m "feat(mobile): add IDE file chip to SessionCard and SessionDetailSheet"
```

---

### Task 7: Verify end-to-end wiring

**Files:** None (verification only)

**Step 1: Full Rust build**

Run: `cargo build`
Expected: Clean build.

**Step 2: Full Rust test suite for touched crates**

Run: `cargo test -p claude-view-core && cargo test -p claude-view-server`
Expected: All pass.

**Step 3: Web build**

Run: `cd apps/web && bun run build`
Expected: Clean build.

**Step 4: Trace the field end-to-end**

Verify the wiring chain exists by grepping:
Run: `grep -rn "last_user_file\|lastUserFile\|ide_file" crates/ packages/ apps/ --include="*.rs" --include="*.ts" --include="*.tsx"`

Expected: Hits in all layers:
- `crates/core/src/live_parser.rs` — `ide_file` field + `strip_noise_tags`
- `crates/core/src/accumulator.rs` — `last_user_file`
- `crates/server/src/live/state.rs` — `last_user_file` on LiveSession
- `crates/server/src/live/manager.rs` — `last_user_file` on JsonlMetadata + apply + accum loops
- `packages/shared/src/types/generated/LiveSession.ts` — `lastUserFile`
- `packages/shared/src/types/relay.ts` — `lastUserFile`
- `apps/web/src/components/live/use-live-sessions.ts` — `lastUserFile`
- `apps/web/src/components/live/SessionCard.tsx` — `session.lastUserFile`
- `apps/mobile/components/SessionCard.tsx` — `session.lastUserFile`
- `apps/mobile/components/SessionDetailSheet.tsx` — `session.lastUserFile`

**Step 5: Manual spot check**

Start the dev server (`bun dev`), open a live session that uses IDE context, and verify:
1. Session card shows clean text (no `<system-reminder>` noise)
2. File chip appears if the user had a file open
3. File chip is absent when no IDE context exists

---

## Changelog of Fixes Applied (Audit → Final Plan)

| # | Issue | Severity | Fix Applied |
| --- | ----- | -------- | ----------- |
| 1 | Plan used `regex::Regex` — codebase only has `regex-lite` | Blocker | Changed to `regex_lite::Regex` in `strip_noise_tags()`. No `Cargo.toml` change needed. |
| 2 | Plan referenced `PerSessionAcc` — actual struct in `manager.rs` is `SessionAccumulator` | Blocker | Replaced all occurrences of `PerSessionAcc` with `SessionAccumulator` |
| 3 | Plan manually edited auto-generated `LiveSession.ts` — would be overwritten by `./scripts/generate-types.sh` | Blocker | Changed Task 4 to run `./scripts/generate-types.sh` after Rust changes; manual edit only as fallback |
| 4 | Plan imported `lucide-react-native` (Step 1) before checking it exists (Step 3) | Blocker | Reordered Task 6: Step 1 now installs the dep, Step 2 writes the import |
| 5 | `LazyLock` MSRV not verified (no `rust-toolchain.toml`) | Warning | Added note to verify `rustc --version` and `OnceLock` fallback for Rust < 1.80 |
| 6 | Mobile SessionCard insertion "line ~35" was inside the XStack, not after it | Warning | Updated to "after the status XStack closing tag (line ~40)" |
| 7 | Task 5 commit did not include `Cargo.toml` (no longer needed) | Minor | Removed `Cargo.toml` from `git add` since `regex-lite` is already a dep |
| 8 | Task 6 commit missing `apps/mobile/package.json` | Minor | Added `apps/mobile/package.json` to `git add` for the new dep |
| 9 | `SectionLabel` nature unclear | Minor | Added note that it's a local component (lines 86–98), not an import |
| 10 | `regex-lite` does NOT support backreferences (`\1`) — regex would panic at runtime | Critical | Rewrote `NOISE_TAGS` regex to enumerate each tag with explicit closing tags using `concat!()` |
| 11 | 5 of 7 `LiveSession` construction sites not covered — would fail to compile | Critical | Added Step 3i covering all 5 missing sites in `hooks.rs`, `terminal.rs`, `sessions.rs` |
| 12 | Used `LazyLock` — codebase convention is `OnceLock` (3 existing uses, 0 `LazyLock`) | Warning | Switched entire implementation to `OnceLock` with `get_or_init()` pattern |
| 13 | `has_system_prefix` reads `content_preview` after stripping — would miss command tags | Warning | Added fix to read RAW content for `has_system_prefix` detection, before stripping |
| 14 | Step ordering in Task 2: variable declaration (f) came after usage (d, e) | Minor | Reordered: declaration (d) now precedes usage (e, f) |
| 15 | Task 3 commit `git add` missing 3 new files (`hooks.rs`, `terminal.rs`, `sessions.rs`) | Minor | Added all 6 modified files to `git add` |
| 16 | Task 4 step numbering gap: 1, 2, 4, 5, 6 after removing manual edit step | Minor | Renumbered to 1, 2, 3, 4, 5 |
| 17 | `LiveLine` error fallback (line ~288) only mentioned in passing, no explicit insertion point | Important | Added explicit instructions: `ide_file: None,` after `skill_names: Vec::new(),` at line ~315 |
| 18 | `lucide-react-native` + `react-native-svg` adds unnecessary native dep for one icon | Warning | Replaced with `@expo/vector-icons` (`Ionicons`) — already installed, zero new deps, Expo-blessed |
| 19 | `has_system_prefix` raw-content fix silently returns `""` for array content — undocumented | Warning | Added code comment explaining `.as_str()` returns `None` for arrays and why this is safe |
| 20 | Plan description says `parse_tail` returns `(Vec<LiveLine>, usize)` — actually `u64` | Minor | Added note in Task 2 Step 2 clarifying the actual return type |
| 21 | `OnceLock<Regex>` is a new pattern (existing regex in `parser.rs`/`discovery.rs` recompiles each call) | Minor | Added note in Task 1 explaining why `OnceLock` is correct here (per-line hot path) despite novel usage |
| 22 | Mobile SessionCard does NOT currently render `lastUserMessage` — plan prose implied it did | Minor | Updated Task 6 Step 2b to state we are adding both message + chip as new rows |
| 23 | Task 6 commit included `bun.lock` / `package.json` — no longer needed after switching to `@expo/vector-icons` | Minor | Removed `package.json` and `bun.lock` from `git add`; commit is now just the two `.tsx` files |
