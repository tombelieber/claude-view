# Claude Rules Reference

Detailed code examples and backstories for the rules in CLAUDE.md. Read on-demand when you need a specific pattern.

---

## Spawning Claude CLI from Rust: Strip All Session Env Vars

**Lesson learned (Feb 2026):** The server spawns `claude -p` to classify sessions. When the server itself runs inside a Claude Code session, the child `claude` process inherits env vars that trigger "nested session" detection and hangs or refuses to start.

Claude Code sets **three** env vars. **Critical: `std::env::vars()` iteration alone is NOT reliable on macOS** — it can miss env vars with empty or unusual values.

```rust
// WRONG — relies solely on std::env::vars() which may miss vars on macOS
let claude_vars: Vec<String> = std::env::vars()
    .filter(|(k, _)| k.starts_with("CLAUDE"))
    .map(|(k, _)| k)
    .collect();
for var in &claude_vars {
    cmd.env_remove(var);
}

// RIGHT — hardcode known vars + catch future ones via iteration
let known_vars = ["CLAUDECODE", "CLAUDE_CODE_SSE_PORT", "CLAUDE_CODE_ENTRYPOINT"];
let extra_vars: Vec<String> = std::env::vars()
    .filter(|(k, _)| k.starts_with("CLAUDE") && !known_vars.contains(&k.as_str()))
    .map(|(k, _)| k)
    .collect();
let mut cmd = TokioCommand::new("claude");
cmd.args(["-p", "--output-format", "json", "--model", &model, &prompt])
    .stdin(std::process::Stdio::null());
for var in known_vars.iter().chain(extra_vars.iter().map(|s| &**s)) {
    cmd.env_remove(var);
}
```

Also strip when launching the server from a Claude Code terminal:

```bash
env -u CLAUDECODE -u CLAUDE_CODE_SSE_PORT -u CLAUDE_CODE_ENTRYPOINT \
  nohup ./target/debug/vibe-recall > /tmp/server.log 2>&1 &
```

`dev:server` script must start with `unset CLAUDECODE CLAUDE_CODE_SSE_PORT CLAUDE_CODE_ENTRYPOINT`.

**Four layers of defense:**

| Layer | What | Protects against |
|-------|------|-----------------|
| Hardcoded `env_remove()` in Rust | Strips 3 known vars by name | `std::env::vars()` missing vars on macOS |
| Dynamic `env_remove()` in Rust | Strips future CLAUDE-prefixed vars | New env vars in future versions |
| `unset` in `dev:server` script | Strips vars before `cargo watch` | `bun dev` started from Claude Code terminal |
| `.stdin(Stdio::null())` in Rust | Prevents child blocking on stdin | Server process chain inheriting stdin |

---

## macOS: `sysinfo` Cannot Read Process `cwd` or `cmd`

**Lesson learned (Feb 2026):** On macOS, `sysinfo` can see process names but **cannot read `cwd` (returns `None`) or `cmd` (returns empty)** due to security restrictions. `ps aux` and `lsof` use different system APIs with broader access.

```rust
// WRONG — cwd is always None on macOS
if let Some(cwd) = process.cwd() {
    result.insert(cwd.to_path_buf(), proc_info);
}

// RIGHT — lsof fallback
let cwd = process
    .cwd()
    .map(|p| p.to_path_buf())
    .or_else(|| get_cwd_via_lsof(pid));
```

`lsof -a -p <pid> -d cwd -Fn` reliably returns cwd for same-user processes. Parse lines starting with `n`. ~10ms per process.

Also: `process.cmd()` returns empty on macOS, so Node.js-based install check (`cmd` args containing `@anthropic-ai/claude`) never matches. Only `name.contains("claude")` works.

---

## Claude Code Project Directory Encoding

**Lesson learned (Feb 2026):** Claude Code encodes paths using `-` replacement, NOT URL encoding. `/Users/foo/dev/@org/project` becomes `-Users-foo-dev--org-project`. `urlencoding::decode()` finds no `%XX` patterns and returns input unchanged.

```rust
// WRONG
let project_path = urlencoding::decode(&encoded_name).to_string();

// RIGHT
let resolved = vibe_recall_core::discovery::resolve_project_path(&encoded_name);
let project_path = resolved.full_path;
```

---

## Tracing: EnvFilter vs with_max_level

**Lesson learned (Feb 2026):** `with_max_level(Level::WARN)` completely ignores `RUST_LOG`.

```rust
// WRONG — ignores RUST_LOG
let subscriber = FmtSubscriber::builder()
    .with_max_level(Level::WARN)
    .finish();

// RIGHT — respects RUST_LOG
let subscriber = FmtSubscriber::builder()
    .with_env_filter(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
    )
    .finish();
```

Scope RUST_LOG: `RUST_LOG=warn,vibe_recall_server=info,vibe_recall_core=info`

---

## Unbounded AI/CLI Process Spawning

**Lesson learned (Feb 2026):** Tier 2 AI classification spawned `claude -p` for every session on startup. 40 JSONL files → 40 concurrent processes → timeouts, rate limits, infinite retry loop.

```rust
// WRONG — spawns N concurrent claude processes on startup
for session in discovered_sessions {
    if session.status == Paused {
        tokio::spawn(classify_with_ai(session));
    }
}

// RIGHT — structural only on discovery, AI only on real transitions
let ai_semaphore = Arc::new(Semaphore::new(1));
if new_status == Paused && old_status == Some(Working) {
    if let Ok(permit) = ai_semaphore.try_acquire() {
        tokio::spawn(async move {
            let _permit = permit;
            classify_with_ai(session).await;
        });
    }
}
```

---

## Full-Stack Wiring Checklist

| Layer | Verify | Command |
|-------|--------|---------|
| DB migration | Column exists | `sqlite3 ... ".schema sessions"` |
| SQL SELECT | Column in query | `grep -rn "FROM sessions" crates/db/src/queries/` |
| Rust struct | Field present | Check `SessionRow`, `SessionInfo` |
| API response | Field in JSON | `curl localhost:47892/api/sessions?limit=1 \| jq .sessions[0].newField` |
| TS type | Field typed | Check `src/types/generated/` |
| Hook | Field returned | Check `use-*.ts` return type |
| Component | Field rendered | Check component JSX |
| **Browser** | **Actually visible** | Open the UI and look |

---

## Rust Performance Rules — Examples

### mmap: never copy

```rust
// WRONG — copies entire file into heap
let mmap = unsafe { Mmap::map(&file)? };
let data = mmap.to_vec();
parse_bytes(&data)

// RIGHT — zero-copy
let mmap = unsafe { Mmap::map(&file)? };
let result = parse_bytes(&mmap);
```

### memmem::Finder: create once

```rust
// WRONG — rebuilds SIMD table per call
fn extract_content(line: &[u8]) -> Option<String> {
    let finder = memmem::Finder::new(b"\"content\":\"");
    ...
}

// RIGHT — create once, pass reference
let content_finder = memmem::Finder::new(b"\"content\":\"");
for line in lines {
    extract_content(line, &content_finder);
}
```

### SQLite: batch writes

```rust
// WRONG — 491 implicit transactions
for session in sessions { db.update_session(&session).await; }

// RIGHT — single transaction
db.batch_update_sessions(&sessions).await;
```

### SIMD pre-filter

```rust
let skill_finder = memmem::Finder::new(b"\"name\":\"Skill\"");
for line in lines {
    if skill_finder.find(line).is_some() {
        if let Ok(v) = serde_json::from_slice::<Value>(line) { ... }
    }
}
```

---

## Statistical Analysis — Examples

### Directional language for signed metrics

```rust
// WRONG — "You're -30% more efficient"
vars.insert("improvement".to_string(), format!("{:.0}", improvement));

// RIGHT — "Your re-edit rate has increased by 30%"
let direction = if improvement > 5.0 { "improved" }
    else if improvement < -5.0 { "increased" }
    else { "stayed stable at" };
vars.insert("direction".to_string(), direction.to_string());
vars.insert("improvement".to_string(), super::format_improvement(improvement.abs()));
```

### Filtering outlier durations

```rust
let valid: Vec<f64> = sessions.iter()
    .filter(|s| s.duration_seconds > 0)
    .map(|s| s.duration_seconds.min(14400) as f64)
    .collect();
```

### Degenerate "zero" bucket

```rust
// WRONG — creates degenerate bucket
let bucket = if s.files_read_count == 0 { "zero" } else if ratio < 1.0 { "low" } ...

// RIGHT — zero falls naturally into "low"
let ratio = s.files_read_count as f64 / s.files_edited_count as f64;
let bucket = if ratio < 1.0 { "low" } else if ratio < 3.0 { "moderate" } else { "high" };
```

---

## React Hook Rules — Examples

### Raw parsed objects in useEffect deps

```tsx
// WRONG — filters is a new {} every render
useEffect(() => {
  if (isOpen) setDraftFilters(filters);
}, [isOpen, filters]);

// RIGHT — memoize on primitive key
const urlKey = searchParams.toString();
const filters = useMemo(() => parseFilters(searchParams), [urlKey]);
```

### URL params coordination

```tsx
// WRONG — wipes other hooks' params
const params = new URLSearchParams();
params.set('myKey', value);
setSearchParams(params);

// RIGHT — preserves existing params
const params = new URLSearchParams(existingSearchParams);
params.set('myKey', value);
setSearchParams(params);
```

### WebSocket stale connection guard

```tsx
// WRONG — stale onclose corrupts new WS
ws.onclose = (event) => {
  if (unmountedRef.current) return
  wsRef.current = null
  if (intentionalCloseRef.current) return
  scheduleReconnect()
}

// RIGHT — stale guard
ws.onclose = (event) => {
  if (unmountedRef.current) return
  if (wsRef.current !== ws) return  // stale close from superseded socket
  wsRef.current = null
  if (intentionalCloseRef.current) return
  scheduleReconnect()
}
```

Why: React effect cleanup and new effect run in same commit phase, but `ws.close()` is async. The old WS's `onclose` fires after the new connection exists.

---

## External Data Sources — Pattern

```rust
// WRONG — trust the index blindly
let sessions = read_index_file("sessions-index.json");

// RIGHT — index + filesystem catch-up
let mut sessions = read_index_file("sessions-index.json");
let indexed_ids: HashSet<_> = sessions.iter().map(|s| &s.id).collect();
for file in scan_directory("*.jsonl") {
    if !indexed_ids.contains(&file.stem()) {
        sessions.push(make_entry_from_file(file));
    }
}
```

---

## SSE / Vite Dev Proxy — Pattern

```tsx
function sseUrl(path: string): string {
  if (window.location.port === '5173') {
    return `http://localhost:47892${path}`  // bypass Vite proxy
  }
  return path
}
```

Pattern for server-to-UI progress:

| Layer | What | File |
|-------|------|------|
| Backend | Atomic progress state | `crates/server/src/indexing_state.rs` |
| Backend | SSE endpoint polling atomics | `crates/server/src/routes/indexing.rs` |
| Frontend | `EventSource` hook | `src/hooks/use-indexing-progress.ts` |
| Frontend | Progress bar component | `src/components/StorageOverview.tsx` |

---

## shadcn/ui CSS Variable Classes

This project does not define shadcn/ui CSS variables. These classes resolve to nothing:

`text-muted-foreground`, `bg-muted`, `text-foreground`, `bg-background`, `text-primary`, `border-border`

```tsx
// WRONG — invisible in dark mode
<span className="text-muted-foreground">Working...</span>

// RIGHT
<span className="text-gray-500 dark:text-gray-400">Working...</span>
```

---

## Release Process — Details

```bash
# From repo root (must be on main branch):
./scripts/release.sh patch   # 0.2.4 -> 0.2.5
./scripts/release.sh minor   # 0.2.4 -> 0.3.0
./scripts/release.sh major   # 0.2.4 -> 1.0.0

# Also bump Cargo.toml workspace version to match:
sed -i '' 's/^version = ".*"/version = "X.Y.Z"/' Cargo.toml
git add Cargo.toml
git commit --amend --no-edit
git push origin main --tags
```

Version sources that must match:

| File | Must match tag |
|------|---------------|
| `npx-cli/package.json` | Primary (bumped by script) |
| `Cargo.toml` (`workspace.package.version`) | Must bump manually |
| Root `package.json` | Informational (keep in sync) |
