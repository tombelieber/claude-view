# Unified Message Categories Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add category field to Rust Message and ToolCall structs so all JSONL message types (hooks, progress, system, snapshots, queue-ops) carry categories through both REST API and WebSocket paths, enabling badge filters across all views.

**Architecture:** Shared `categorize_tool()` function in `crates/core/src/category.rs` called by both the REST parser (`parser.rs`) and WebSocket handler (`terminal.rs`). Frontend removes local `categorizeTool()` and reads category from API responses. ConversationView already renders through `HistoryRichPane` → `RichPane`, so badge filters appear automatically once RichMessages carry categories.

**Tech Stack:** Rust (serde, ts-rs), TypeScript/React, Tailwind CSS

---

### Task 1: Add `category` module to Rust core crate

**Files:**
- Create: `crates/core/src/category.rs`
- Modify: `crates/core/src/lib.rs`

**Step 1: Create the category module**

```rust
// crates/core/src/category.rs

/// Categorize a tool call by its name.
///
/// Maps tool names to categories:
/// - "Skill" → "skill"
/// - "mcp__*" / "mcp_*" → "mcp"
/// - "Task" → "agent"
/// - everything else → "builtin"
pub fn categorize_tool(name: &str) -> &'static str {
    if name == "Skill" {
        "skill"
    } else if name.starts_with("mcp__") || name.starts_with("mcp_") {
        "mcp"
    } else if name == "Task" {
        "agent"
    } else {
        "builtin"
    }
}

/// Categorize a progress message by its `data.type` subfield.
///
/// Maps progress subtypes to categories:
/// - "hook_progress" → "hook"
/// - "agent_progress" → "agent"
/// - "bash_progress" → "builtin"
/// - "mcp_progress" → "mcp"
/// - "waiting_for_task" → "agent"
/// - anything else → None (uncategorized)
pub fn categorize_progress(data_type: &str) -> Option<&'static str> {
    match data_type {
        "hook_progress" => Some("hook"),
        "agent_progress" => Some("agent"),
        "bash_progress" => Some("builtin"),
        "mcp_progress" => Some("mcp"),
        "waiting_for_task" => Some("agent"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_tool_skill() {
        assert_eq!(categorize_tool("Skill"), "skill");
    }

    #[test]
    fn test_categorize_tool_mcp() {
        assert_eq!(categorize_tool("mcp__chrome-devtools__click"), "mcp");
        assert_eq!(categorize_tool("mcp_playwright"), "mcp");
    }

    #[test]
    fn test_categorize_tool_agent() {
        assert_eq!(categorize_tool("Task"), "agent");
    }

    #[test]
    fn test_categorize_tool_builtin() {
        assert_eq!(categorize_tool("Read"), "builtin");
        assert_eq!(categorize_tool("Bash"), "builtin");
        assert_eq!(categorize_tool("Edit"), "builtin");
        assert_eq!(categorize_tool("Write"), "builtin");
        assert_eq!(categorize_tool("Grep"), "builtin");
        assert_eq!(categorize_tool("Glob"), "builtin");
    }

    #[test]
    fn test_categorize_progress_hook() {
        assert_eq!(categorize_progress("hook_progress"), Some("hook"));
    }

    #[test]
    fn test_categorize_progress_agent() {
        assert_eq!(categorize_progress("agent_progress"), Some("agent"));
        assert_eq!(categorize_progress("waiting_for_task"), Some("agent"));
    }

    #[test]
    fn test_categorize_progress_builtin() {
        assert_eq!(categorize_progress("bash_progress"), Some("builtin"));
    }

    #[test]
    fn test_categorize_progress_mcp() {
        assert_eq!(categorize_progress("mcp_progress"), Some("mcp"));
    }

    #[test]
    fn test_categorize_progress_unknown() {
        assert_eq!(categorize_progress("unknown_progress"), None);
    }
}
```

**Step 2: Register module in lib.rs**

Add `pub mod category;` to `crates/core/src/lib.rs` (after `pub mod classification;`).
Also add `pub use category::*;`.

**Step 3: Run tests to verify**

Run: `cargo test -p claude-view-core -- category`
Expected: All tests pass

**Step 4: Commit**

```bash
git add crates/core/src/category.rs crates/core/src/lib.rs
git commit -m "feat: add shared message category module"
```

---

### Task 2: Add `category` field to ToolCall and Message structs

**Files:**
- Modify: `crates/core/src/types.rs:44-130` (ToolCall struct, Message struct, and builders)

**Step 1: Write the failing tests**

Add to the existing tests in `crates/core/src/types.rs`:

```rust
#[test]
fn test_toolcall_with_category() {
    let tc = ToolCall { name: "Bash".to_string(), count: 1, input: None, category: Some("builtin".to_string()) };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(json.contains("\"category\":\"builtin\""));
}

#[test]
fn test_toolcall_category_omitted_when_none() {
    let tc = ToolCall { name: "Bash".to_string(), count: 1, input: None, category: None };
    let json = serde_json::to_string(&tc).unwrap();
    assert!(!json.contains("category"));
}

#[test]
fn test_message_with_category() {
    let msg = Message::tool_use("")
        .with_category("builtin");
    assert_eq!(msg.category, Some("builtin".to_string()));

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"category\":\"builtin\""));
}

#[test]
fn test_message_category_omitted_when_none() {
    let msg = Message::user("Hello");
    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.contains("category"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core -- test_toolcall_with_category test_message_with_category`
Expected: FAIL — fields don't exist

**Step 3: Add category field to ToolCall struct**

In `crates/core/src/types.rs`, add to the `ToolCall` struct (after `input`):

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
```

**Step 4: Add category field to Message struct**

Add to the `Message` struct (after `metadata`):

```rust
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
```

In `new_with_role()`, add:
```rust
    category: None,
```

Add builder method (after `with_metadata`):
```rust
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
```

**Step 5: Fix any compilation errors**

Any code constructing `ToolCall` with struct literals needs `category: None` added. Check `parser.rs` `extract_assistant_content()` (around line 523-528) — add `category: None` to the `ToolCall` construction there.

**Step 6: Run tests to verify they pass**

Run: `cargo test -p claude-view-core -- test_toolcall_with_category test_toolcall_category_omitted test_message_with_category test_message_category_omitted`
Expected: PASS

**Step 7: Run full crate tests**

Run: `cargo test -p claude-view-core`
Expected: All tests pass

**Step 8: Commit**

```bash
git add crates/core/src/types.rs
git commit -m "feat: add category field to ToolCall and Message structs"
```

---

### Task 3: Assign categories in the REST API parser

**Files:**
- Modify: `crates/core/src/parser.rs:1-396`

**Step 1: Write the failing test**

Add to the existing tests in `crates/core/src/parser.rs` (use `all_types.jsonl`):

```rust
#[tokio::test]
async fn test_parse_tool_use_has_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    // Index 3: tool-only assistant (Edit) → ToolUse role
    let msg = &session.messages[3];
    assert_eq!(msg.role, Role::ToolUse);
    assert_eq!(msg.category, Some("builtin".to_string()));

    // Each tool_call should also carry its own category
    let tc = &msg.tool_calls.as_ref().unwrap()[0];
    assert_eq!(tc.category, Some("builtin".to_string()));
}

#[tokio::test]
async fn test_parse_progress_hook_has_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    // Index 6: progress (hook_progress) → category "hook"
    let msg = &session.messages[6];
    assert_eq!(msg.role, Role::Progress);
    assert_eq!(msg.category, Some("hook".to_string()));
}

#[tokio::test]
async fn test_parse_system_has_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    // Index 5: system (turn_duration) → category "system"
    let msg = &session.messages[5];
    assert_eq!(msg.role, Role::System);
    assert_eq!(msg.category, Some("system".to_string()));
}

#[tokio::test]
async fn test_parse_queue_op_has_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    // Index 7: queue-operation → category "queue"
    let msg = &session.messages[7];
    assert_eq!(msg.category, Some("queue".to_string()));
}

#[tokio::test]
async fn test_parse_file_snapshot_has_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    // Index 10: file-history-snapshot → category "snapshot"
    let msg = &session.messages[10];
    assert_eq!(msg.category, Some("snapshot".to_string()));
}

#[tokio::test]
async fn test_parse_user_has_no_category() {
    let path = fixtures_path().join("all_types.jsonl");
    let session = parse_session(&path).await.unwrap();

    let msg = &session.messages[0];
    assert_eq!(msg.role, Role::User);
    assert_eq!(msg.category, None);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p claude-view-core -- test_parse_tool_use_has_category test_parse_progress_hook_has_category test_parse_system_has_category`
Expected: FAIL — category is always None

**Step 3: Add category assignment to parser.rs**

Add import at top of `parser.rs`:
```rust
use crate::category::{categorize_tool, categorize_progress};
```

In `extract_assistant_content()`, where each `ToolCall` is constructed (around line 523-528), set category per tool call:

```rust
ToolCall {
    name: name.clone(),
    count: 1,
    input: input.clone(),
    category: Some(categorize_tool(&name).to_string()),
}
```

In the `"assistant"` match arm, after building the message and before `messages.push(message)`, set message-level category from the first tool call:
```rust
// Assign message-level category based on first tool call
if has_tools {
    if let Some(first_tool) = tool_calls.first() {
        message = message.with_category(categorize_tool(&first_tool.name));
    }
}
```

In the `"system"` match arm, before `messages.push(message)`:
```rust
let message = message.with_category("system");
```

In the `"progress"` match arm, before `messages.push(message)`:
```rust
let message = if let Some(cat) = categorize_progress(data_type) {
    message.with_category(cat)
} else {
    message
};
```

In the `"queue-operation"` match arm, before `messages.push(message)`:
```rust
let message = message.with_category("queue");
```

In the `"file-history-snapshot"` match arm, before `messages.push(message)`:
```rust
let message = message.with_category("snapshot");
```

In the `"saved_hook_context"` match arm, before `messages.push(message)`:
```rust
let message = message.with_category("hook");
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p claude-view-core -- test_parse`
Expected: All pass

**Step 5: Run full crate tests**

Run: `cargo test -p claude-view-core`
Expected: All pass

**Step 6: Commit**

```bash
git add crates/core/src/parser.rs
git commit -m "feat: assign categories to all message types in REST parser"
```

---

### Task 4: Add category to WebSocket rich mode output

**Files:**
- Modify: `crates/server/src/routes/terminal.rs:359-548` (format_line_for_mode)

**Step 1: Add import**

At the top of `terminal.rs`, add:
```rust
use claude_view_core::category::{categorize_tool, categorize_progress};
```

**Step 2: Add category to tool_use blocks in format_line_for_mode**

In the `"tool_use"` match arm inside the blocks loop (around line 477-494), add `category`:

Change from:
```rust
let mut result = serde_json::json!({
    "type": "tool_use",
    "name": tool_name,
    "input": input,
});
```

To:
```rust
let category = categorize_tool(tool_name);
let mut result = serde_json::json!({
    "type": "tool_use",
    "name": tool_name,
    "input": input,
    "category": category,
});
```

**Step 3: Add category to tool_result blocks**

In the `"tool_result"` match arm (around line 496-511), the WebSocket currently emits tool_result with no category. Track the last tool category and pass it through:

Before the blocks loop, add a tracking variable:
```rust
let mut last_tool_category: Option<&str> = None;
```

In the `"tool_use"` arm (after computing `category`), save it:
```rust
last_tool_category = Some(category);
```

In the `"tool_result"` arm, include the tracked category:
```rust
"tool_result" => {
    let content = block
        .get("content")
        .map(|c| match c {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let mut result = serde_json::json!({
        "type": "tool_result",
        "content": content,
    });
    if let Some(cat) = last_tool_category {
        result["category"] = serde_json::Value::String(cat.to_string());
    }
    if let Some(ts) = timestamp {
        result["ts"] = serde_json::Value::String(ts.to_string());
    }
    results.push(result.to_string());
}
```

**Step 4: Stop skipping progress and system lines**

Currently lines 390-393 skip progress and file-history-snapshot:
```rust
match line_type {
    "progress" | "file-history-snapshot" => return vec![],
    _ => {}
}
```

Change to emit them with categories:
```rust
match line_type {
    "progress" => {
        let data = parsed.get("data");
        let data_type = data
            .and_then(|d| d.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        let category = categorize_progress(data_type);

        let hook_name = data.and_then(|d| d.get("hookName")).and_then(|v| v.as_str());
        let command = data.and_then(|d| d.get("command")).and_then(|v| v.as_str());
        let content = if let Some(hn) = hook_name {
            format!("{}: {}", data_type, hn)
        } else if let Some(cmd) = command {
            format!("{}: {}", data_type, cmd)
        } else {
            data_type.to_string()
        };

        let mut result = serde_json::json!({
            "type": "progress",
            "content": content,
            "metadata": data,
        });
        if let Some(cat) = category {
            result["category"] = serde_json::Value::String(cat.to_string());
        }
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        return vec![result.to_string()];
    }
    "file-history-snapshot" => {
        let mut result = serde_json::json!({
            "type": "system",
            "content": "file-history-snapshot",
            "category": "snapshot",
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        return vec![result.to_string()];
    }
    "system" => {
        let subtype = parsed.get("subtype").and_then(|v| v.as_str()).unwrap_or("unknown");
        let duration_ms = parsed.get("durationMs").and_then(|v| v.as_u64());
        let content = if let Some(ms) = duration_ms {
            format!("{}: {}ms", subtype, ms)
        } else {
            subtype.to_string()
        };
        let mut result = serde_json::json!({
            "type": "system",
            "content": content,
            "category": "system",
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        return vec![result.to_string()];
    }
    "queue-operation" => {
        let operation = parsed.get("operation").and_then(|v| v.as_str()).unwrap_or("unknown");
        let mut result = serde_json::json!({
            "type": "system",
            "content": format!("queue-{}", operation),
            "category": "queue",
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        return vec![result.to_string()];
    }
    "summary" => {
        let summary_text = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let mut result = serde_json::json!({
            "type": "summary",
            "content": summary_text,
        });
        if let Some(ts) = timestamp {
            result["ts"] = serde_json::Value::String(ts.to_string());
        }
        return vec![result.to_string()];
    }
    _ => {}
}
```

**Step 5: Compile and verify**

Run: `cargo build -p claude-view-server`
Expected: Compiles successfully

**Step 6: Commit**

```bash
git add crates/server/src/routes/terminal.rs
git commit -m "feat: emit categories in WebSocket rich mode for all message types"
```

---

### Task 5: Regenerate TypeScript types

**Files:**
- Modify: `src/types/generated/Message.ts` (auto-generated)
- Modify: `src/types/generated/ToolCall.ts` (auto-generated)

**Step 1: Run ts-rs export**

Run: `cargo test -p claude-view-core -- export_bindings`

If that doesn't work, run: `cargo test -p claude-view-core`

The `#[ts(export)]` attributes on Message and ToolCall will auto-generate the TS types.

**Step 2: Verify the generated files include `category`**

Check that `src/types/generated/Message.ts` now includes:
```typescript
category?: string;
```

Check that `src/types/generated/ToolCall.ts` now includes:
```typescript
category?: string;
```

**Step 3: Commit**

```bash
git add src/types/generated/
git commit -m "chore: regenerate TS types with category field on Message and ToolCall"
```

---

### Task 6: Expand ActionCategory and update frontend types

**Files:**
- Modify: `src/components/live/action-log/types.ts:1`
- Modify: `src/components/live/action-log/ActionFilterChips.tsx:4-12`
- Delete: `src/lib/categorize-tool.ts`

**Step 1: Expand ActionCategory type**

In `src/components/live/action-log/types.ts`, change line 1:

```typescript
export type ActionCategory = 'skill' | 'mcp' | 'builtin' | 'agent' | 'hook' | 'error' | 'system' | 'snapshot' | 'queue'
```

**Step 2: Add new categories to ActionFilterChips**

In `src/components/live/action-log/ActionFilterChips.tsx`, update the CATEGORIES array:

```typescript
const CATEGORIES: { id: ActionCategory | 'all'; label: string; color: string }[] = [
  { id: 'all', label: 'All', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'builtin', label: 'Builtin', color: 'bg-gray-500/10 text-gray-400 border-gray-500/30' },
  { id: 'mcp', label: 'MCP', color: 'bg-blue-500/10 text-blue-400 border-blue-500/30' },
  { id: 'agent', label: 'Agent', color: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30' },
  { id: 'skill', label: 'Skill', color: 'bg-purple-500/10 text-purple-400 border-purple-500/30' },
  { id: 'hook', label: 'Hook', color: 'bg-amber-500/10 text-amber-400 border-amber-500/30' },
  { id: 'system', label: 'System', color: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30' },
  { id: 'snapshot', label: 'Snapshot', color: 'bg-teal-500/10 text-teal-400 border-teal-500/30' },
  { id: 'queue', label: 'Queue', color: 'bg-orange-500/10 text-orange-400 border-orange-500/30' },
  { id: 'error', label: 'Error', color: 'bg-red-500/10 text-red-400 border-red-500/30' },
]
```

**Step 3: Delete categorize-tool.ts**

```bash
rm src/lib/categorize-tool.ts
```

**Step 4: Verify no remaining imports**

Search for `from.*categorize-tool` across the codebase. Update any imports that reference it (they'll be fixed in Task 7 and 8).

**Step 5: Commit**

```bash
git add src/components/live/action-log/types.ts src/components/live/action-log/ActionFilterChips.tsx
git rm src/lib/categorize-tool.ts
git commit -m "feat: expand ActionCategory with system/snapshot/queue, remove categorize-tool.ts"
```

---

### Task 7: Update message-to-rich.ts to use API categories

**Files:**
- Modify: `src/lib/message-to-rich.ts`

**Step 1: Remove categorizeTool import**

Remove:
```typescript
import { categorizeTool } from './categorize-tool'
```

**Step 2: Update tool_use case to read category from each ToolCall**

The generated `ToolCall` type now has `category?: string`. Each ToolCall carries its own category from the Rust parser, so we read it per tool call (preserving correct per-tool categorization for multi-tool messages):

```typescript
case 'tool_use': {
    const toolCalls = msg.tool_calls ?? []
    if (toolCalls.length === 0) {
        // Fallback: legacy data without individual tool calls
        const inputStr = msg.content || ''
        result.push({
            type: 'tool_use',
            content: '',
            name: 'tool',
            input: inputStr || undefined,
            inputData: inputStr ? tryParseJson(inputStr) : undefined,
            ts,
            category: (msg.category as ActionCategory) ?? 'builtin',
        })
        lastToolCategory = (msg.category as ActionCategory) ?? 'builtin'
    } else {
        for (const tc of toolCalls) {
            const inputData = tc.input ?? undefined
            const inputStr = inputData ? JSON.stringify(inputData, null, 2) : undefined
            const category = (tc.category as ActionCategory) ?? 'builtin'
            result.push({
                type: 'tool_use',
                content: '',
                name: tc.name,
                input: inputStr,
                inputData,
                ts,
                category,
            })
            lastToolCategory = category
        }
    }
    break
}
```

**Step 3: Add category to system/progress/summary cases**

In the `system` case, add category:
```typescript
case 'system': {
    const content = stripCommandTags(msg.content)
    result.push({
        type: 'system',
        content: content || '',
        ts,
        metadata: msg.metadata ?? undefined,
        category: (msg.category as ActionCategory) ?? undefined,
    })
    break
}
```

Same for `progress`:
```typescript
case 'progress': {
    const content = stripCommandTags(msg.content)
    result.push({
        type: 'progress',
        content: content || '',
        ts,
        metadata: msg.metadata ?? undefined,
        category: (msg.category as ActionCategory) ?? undefined,
    })
    break
}
```

**Step 4: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 5: Update existing test if any**

If `src/lib/message-to-rich.test.ts` exists and references `categorizeTool`, update it.

**Step 6: Commit**

```bash
git add src/lib/message-to-rich.ts
git commit -m "feat: use API-provided categories in messagesToRichMessages"
```

---

### Task 8: Update WebSocket message parsing in RichPane.tsx

**Files:**
- Modify: `src/components/live/RichPane.tsx`

**Step 1: Remove categorizeTool import and update parseRichMessage**

Remove the `categorizeTool` import from RichPane.tsx.

In `parseRichMessage`, change the `tool_use` case:

From:
```typescript
category: categorizeTool(msg.name),
```

To:
```typescript
category: (msg.category as ActionCategory) ?? 'builtin',
```

**Step 2: Add category to tool_result in parseRichMessage**

The WebSocket now emits `category` on tool_result messages (from Task 4). Update the tool_result handler:

From:
```typescript
if (msg.type === 'tool_result') {
    const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content || '', null, 2))
    if (!content.trim()) return null
    return {
        type: 'tool_result',
        content,
        ts: parseTimestamp(msg.ts),
    }
}
```

To:
```typescript
if (msg.type === 'tool_result') {
    const content = stripCommandTags(typeof msg.content === 'string' ? msg.content : JSON.stringify(msg.content || '', null, 2))
    if (!content.trim()) return null
    return {
        type: 'tool_result',
        content,
        ts: parseTimestamp(msg.ts),
        category: (msg.category as ActionCategory) ?? undefined,
    }
}
```

**Step 3: Add handlers for new message types**

Add handlers for progress, system, and summary types in `parseRichMessage` (before the final `return null`):

```typescript
if (msg.type === 'progress') {
    return {
        type: 'progress',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
        metadata: msg.metadata,
        category: (msg.category as ActionCategory) ?? undefined,
    }
}
if (msg.type === 'system') {
    return {
        type: 'system',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
        category: (msg.category as ActionCategory) ?? 'system',
    }
}
if (msg.type === 'summary') {
    return {
        type: 'summary',
        content: typeof msg.content === 'string' ? msg.content : '',
        ts: parseTimestamp(msg.ts),
    }
}
```

**Step 4: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 5: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: read categories from WebSocket messages, handle new types"
```

---

### Task 9: Update RichPane category counts and verbose filter

**Files:**
- Modify: `src/components/live/RichPane.tsx:785-819`

**Step 1: Update categoryCounts initialization**

Change:
```typescript
const counts: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, error: 0 }
```

To:
```typescript
const counts: Record<ActionCategory, number> = { skill: 0, mcp: 0, builtin: 0, agent: 0, hook: 0, error: 0, system: 0, snapshot: 0, queue: 0 }
```

**Step 2: Update verbose filter logic**

In `displayMessages`, the verbose filter currently always shows system/progress/summary:
```typescript
if (m.type === 'system' || m.type === 'progress' || m.type === 'summary') return true
```

Change to check category:
```typescript
// Structural types: show if filter is 'all' OR if their category matches the filter
if (m.type === 'system' || m.type === 'progress' || m.type === 'summary') {
    return !m.category || m.category === verboseFilter
}
```

This means:
- When filter is 'all': all messages shown (the early return handles this)
- When filter is 'hook': only hook-category progress messages shown, not system/snapshot
- When filter is 'system': only system-category messages shown

**Step 3: Verify TypeScript compiles**

Run: `bunx tsc --noEmit`

**Step 4: Commit**

```bash
git add src/components/live/RichPane.tsx
git commit -m "feat: update category counts and verbose filter for new categories"
```

---

### Task 10: End-to-end verification

**Files:** None (testing only)

**Step 1: Run all Rust tests**

Run: `cargo test -p claude-view-core -p claude-view-server`
Expected: All pass

**Step 2: Run TypeScript type check**

Run: `bunx tsc --noEmit`
Expected: No errors

**Step 3: Run frontend tests**

Run: `bun test`
Expected: All pass (including message-to-rich.test.ts if it exists)

**Step 4: Build the server**

Run: `cargo build -p claude-view-server`
Expected: Compiles

**Step 5: Manual verification**

Start the dev server and open session `c3393d11-763d-4636-bf86-0c18158864c2`.

Check:
1. ConversationView verbose mode shows badge filter bar (via HistoryRichPane → RichPane — no extra wiring needed)
2. Hook badge shows non-zero count (should be ~144 from hook_progress entries)
3. System badge shows non-zero count
4. Clicking "Hook" badge filters to only hook messages
5. Live terminal shows same badge behavior
6. In a multi-tool message, each tool_use shows its own correct category (e.g. Bash=builtin, mcp__*=mcp)
7. tool_result messages inherit category from their preceding tool_use in both REST and WebSocket views

**Step 6: Final commit if any fixups needed**

```bash
git add -A
git commit -m "fix: end-to-end verification fixups"
```
