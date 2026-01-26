---
status: pending
date: 2026-01-27
---

# Phase 2: Backend Swap + UI Enhancement

> **Reality:** Rust backend is production-ready (120 tests). Node was a prototype. Just swap and enhance UI.

## Overview

| Component | Before | After |
|-----------|--------|-------|
| Backend | Node/Express (toy) | Rust/Axum (production) |
| API | Basic 2 endpoints | Same + health + better data |
| UI | Basic display | Enhanced with new features |

**New Rust features not yet in UI:**
- Tool counts (Read: 5, Edit: 3, Bash: 2, Write: 1)
- Skills used (/commit, /review-pr, /brainstorm)
- Better previews (cleaned, truncated properly)
- Health endpoint for status

---

## Task 1: Add Static File Serving to Rust

**Goal:** Rust serves the React frontend (already done in Phase 1 plan, just execute).

**Files:**
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/src/main.rs`

**Changes:**

```rust
// crates/server/src/lib.rs - add to imports
use std::path::PathBuf;
use tower_http::services::{ServeDir, ServeFile};

/// Create app with optional static file serving.
pub fn create_app_with_static(static_dir: Option<PathBuf>) -> Router {
    let state = AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut app = Router::new()
        .merge(api_routes(state))
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Serve static files with SPA fallback
    if let Some(dir) = static_dir {
        let index = dir.join("index.html");
        app = app.fallback_service(
            ServeDir::new(&dir).not_found_service(ServeFile::new(&index))
        );
    }

    app
}
```

```rust
// crates/server/src/main.rs - update main
let static_dir = std::env::var("STATIC_DIR")
    .ok()
    .map(PathBuf::from)
    .or_else(|| {
        let dist = PathBuf::from("dist");
        dist.exists().then_some(dist)
    });

let app = match static_dir {
    Some(ref dir) => {
        info!("Serving static files from: {:?}", dir);
        create_app_with_static(Some(dir.clone()))
    }
    None => {
        info!("API-only mode (no static dir)");
        create_app()
    }
};
```

**Verification:**
```bash
bun run build              # Build React
cargo run -p vibe-recall-server  # Run Rust
# Visit http://localhost:47892 → should see app
```

**Commit:** `feat(server): add static file serving with SPA fallback`

---

## Task 2: Remove Node Backend

**Goal:** Delete the prototype code.

```bash
rm -rf src/server/
```

**Update package.json:**
```json
{
  "scripts": {
    "dev": "concurrently \"cargo run -p vibe-recall-server\" \"vite --port 5173\"",
    "build": "vite build",
    "preview": "cargo run -p vibe-recall-server",
    "test": "cargo test --workspace && vitest run"
  }
}
```

**Update vite.config.ts for dev proxy:**
```typescript
export default defineConfig({
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:47892',
    },
  },
})
```

**Commit:** `refactor: remove Node backend, Rust is primary`

---

## Task 3: Smoke Test with Browser Agent

**Goal:** Verify app works after swap using Playwright.

**Sub-agent task:**
1. Start Rust server with built frontend
2. Open browser to http://localhost:47892
3. Screenshot home page
4. Click a project (if any)
5. Screenshot project view
6. Click a session (if any)
7. Screenshot session view
8. Report any errors

**Quick Playwright setup:**
```bash
bun add -D @playwright/test
bunx playwright install chromium
```

**Smoke test file:**
```typescript
// e2e/smoke.spec.ts
import { test, expect } from '@playwright/test'

test('app loads and navigates', async ({ page }) => {
  await page.goto('/')
  await page.screenshot({ path: 'e2e/01-home.png' })

  // Click first project if exists
  const project = page.locator('[class*="project"]').first()
  if (await project.isVisible()) {
    await project.click()
    await page.waitForTimeout(1000)
    await page.screenshot({ path: 'e2e/02-project.png' })

    // Click first session if exists
    const session = page.locator('[class*="session"]').first()
    if (await session.isVisible()) {
      await session.click()
      await page.waitForTimeout(1000)
      await page.screenshot({ path: 'e2e/03-session.png' })
    }
  }

  // No console errors
  const errors: string[] = []
  page.on('console', msg => {
    if (msg.type() === 'error') errors.push(msg.text())
  })
  expect(errors).toHaveLength(0)
})
```

**Run:**
```bash
bun run build
cargo run -p vibe-recall-server &
bunx playwright test e2e/smoke.spec.ts
```

**Commit:** `test(e2e): add smoke test for backend swap verification`

---

## Task 4: UI Enhancement - Show Tool Counts

**Goal:** Display tool usage stats that Rust now provides.

**Current SessionInfo from Rust:**
```typescript
interface SessionInfo {
  // ... existing fields ...
  tool_counts: {
    read: number
    edit: number
    bash: number
    write: number
  }
  skills_used: string[]
}
```

**UI Changes:**

```tsx
// In session list item, add tool badges:
<div className="tool-counts">
  {session.tool_counts.read > 0 && <Badge>Read: {session.tool_counts.read}</Badge>}
  {session.tool_counts.edit > 0 && <Badge>Edit: {session.tool_counts.edit}</Badge>}
  {session.tool_counts.bash > 0 && <Badge>Bash: {session.tool_counts.bash}</Badge>}
  {session.tool_counts.write > 0 && <Badge>Write: {session.tool_counts.write}</Badge>}
</div>

// Show skills used:
{session.skills_used.length > 0 && (
  <div className="skills">
    {session.skills_used.map(skill => <Chip key={skill}>/{skill}</Chip>)}
  </div>
)}
```

**Commit:** `feat(ui): display tool counts and skills from Rust backend`

---

## Task 5: UI Enhancement - Health Indicator

**Goal:** Show backend health status in UI header.

```tsx
// components/HealthIndicator.tsx
export function HealthIndicator() {
  const { data, isError } = useQuery({
    queryKey: ['health'],
    queryFn: () => fetch('/api/health').then(r => r.json()),
    refetchInterval: 30000,
  })

  if (isError) return <span className="status-dot red" title="Backend offline" />
  if (data?.status === 'ok') return <span className="status-dot green" title="Backend online" />
  return <span className="status-dot yellow" title="Checking..." />
}
```

**Commit:** `feat(ui): add health indicator for backend status`

---

## Execution Order

```
Task 1 → Task 2 → Task 3 (must pass) → Task 4 → Task 5
         ↑                    │
         │                    ↓
         └── If fail, fix before continuing
```

## Success Criteria

- [ ] Rust serves React app
- [ ] Node backend removed
- [ ] Smoke test passes with screenshots
- [ ] Tool counts visible in UI
- [ ] Skills badges visible in UI
- [ ] Health indicator works
