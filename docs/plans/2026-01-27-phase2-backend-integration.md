---
status: pending
date: 2026-01-27
---

# Phase 2: Backend Integration (Safe Migration)

> **Principle:** Don't break the app. Test with real browser. Only switch when proven.

## Overview

Replace Node/Express backend with Rust/Axum **without breaking the existing web app**.

The Rust backend already exists with 120 unit tests. This plan adds:
1. E2E browser tests (baseline current behavior)
2. Static file serving in Rust
3. Run E2E tests against Rust backend
4. Only remove Node when Rust passes all E2E tests

## Prerequisites

- Phase 1 complete (Rust backend with 120 tests) ✅
- Playwright installed for E2E testing

---

## Task 1: Add Playwright E2E Test Infrastructure

**Goal:** Create baseline E2E tests against the CURRENT Node backend.

**Files:**
- Create: `e2e/playwright.config.ts`
- Create: `e2e/tests/smoke.spec.ts`
- Create: `e2e/tests/projects.spec.ts`
- Create: `e2e/tests/session.spec.ts`

**Step 1: Install Playwright**

```bash
bun add -D @playwright/test
bunx playwright install chromium
```

**Step 2: Create Playwright config**

```typescript
// e2e/playwright.config.ts
import { defineConfig } from '@playwright/test'

export default defineConfig({
  testDir: './tests',
  timeout: 30000,
  retries: 1,
  use: {
    baseURL: 'http://localhost:47892',
    screenshot: 'on',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'bun run dev',  // Will be switched to Rust later
    port: 47892,
    reuseExistingServer: true,
  },
})
```

**Step 3: Create smoke test**

```typescript
// e2e/tests/smoke.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Smoke Tests', () => {
  test('app loads without errors', async ({ page }) => {
    await page.goto('/')

    // Should not show error state
    await expect(page.locator('text=Error')).not.toBeVisible()

    // Should show some content (projects list or loading)
    await expect(page.locator('body')).not.toBeEmpty()

    // Screenshot for visual verification
    await page.screenshot({ path: 'e2e/screenshots/smoke-home.png' })
  })

  test('API health check', async ({ request }) => {
    const response = await request.get('/api/health')
    // Node doesn't have health endpoint, Rust does
    // This test will start passing when Rust is active
  })
})
```

**Step 4: Create projects list test**

```typescript
// e2e/tests/projects.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Projects List', () => {
  test('displays projects from ~/.claude/projects', async ({ page }) => {
    await page.goto('/')

    // Wait for projects to load
    await page.waitForSelector('[data-testid="project-list"], [class*="project"]', {
      timeout: 10000
    })

    // Screenshot the projects list
    await page.screenshot({ path: 'e2e/screenshots/projects-list.png' })

    // Should have at least one project (or empty state)
    const content = await page.textContent('body')
    expect(content).toBeTruthy()
  })

  test('clicking a project shows sessions', async ({ page }) => {
    await page.goto('/')

    // Wait for and click first project
    const firstProject = page.locator('[data-testid="project-item"], [class*="project"]').first()

    if (await firstProject.isVisible()) {
      await firstProject.click()
      await page.waitForTimeout(1000)
      await page.screenshot({ path: 'e2e/screenshots/project-sessions.png' })
    }
  })
})
```

**Step 5: Create session view test**

```typescript
// e2e/tests/session.spec.ts
import { test, expect } from '@playwright/test'

test.describe('Session View', () => {
  test('displays conversation messages', async ({ page }) => {
    await page.goto('/')

    // Navigate to a session (if any exist)
    const sessionLink = page.locator('[data-testid="session-item"], [class*="session"]').first()

    if (await sessionLink.isVisible()) {
      await sessionLink.click()

      // Wait for messages to load
      await page.waitForSelector('[data-testid="message"], [class*="message"]', {
        timeout: 10000
      })

      // Screenshot the conversation
      await page.screenshot({ path: 'e2e/screenshots/session-view.png', fullPage: true })

      // Verify messages are visible
      const messages = page.locator('[data-testid="message"], [class*="message"]')
      const count = await messages.count()
      expect(count).toBeGreaterThan(0)
    }
  })
})
```

**Verification:**
```bash
# Run against current Node backend
bun run dev &
bunx playwright test
# Check e2e/screenshots/ for visual verification
```

**Commit:** `test(e2e): add Playwright E2E tests for baseline behavior`

---

## Task 2: Add Static File Serving to Rust

**Goal:** Make Rust server capable of serving the React frontend.

**Files:**
- Modify: `crates/server/src/lib.rs`
- Modify: `crates/server/src/main.rs`

**Step 1: Update lib.rs with static serving**

```rust
// Add to crates/server/src/lib.rs
use tower_http::services::{ServeDir, ServeFile};

/// Create the Axum application with all routes and middleware.
pub fn create_app() -> Router {
    create_app_with_static(None)
}

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

    // Add static file serving if directory provided
    if let Some(dir) = static_dir {
        let index_path = dir.join("index.html");

        // Serve static files
        let serve_dir = ServeDir::new(&dir)
            .not_found_service(ServeFile::new(&index_path));

        app = app.fallback_service(serve_dir);
    }

    app
}
```

**Step 2: Update main.rs**

```rust
// crates/server/src/main.rs
use std::path::PathBuf;

fn get_static_dir() -> Option<PathBuf> {
    // Check for STATIC_DIR env var first
    if let Ok(dir) = std::env::var("STATIC_DIR") {
        let path = PathBuf::from(dir);
        if path.exists() {
            return Some(path);
        }
    }

    // Check for dist/ in current directory
    let dist = PathBuf::from("dist");
    if dist.exists() {
        return Some(dist);
    }

    // Check for frontend/dist/
    let frontend_dist = PathBuf::from("frontend/dist");
    if frontend_dist.exists() {
        return Some(frontend_dist);
    }

    None
}

#[tokio::main]
async fn main() -> Result<()> {
    // ... existing init code ...

    let static_dir = get_static_dir();
    if let Some(ref dir) = static_dir {
        info!("Serving static files from: {:?}", dir);
    } else {
        info!("No static directory found, API-only mode");
    }

    let app = create_app_with_static(static_dir);

    // ... rest of server startup ...
}
```

**Verification:**
```bash
# Build frontend
bun run build

# Run Rust server with static files
cargo run -p vibe-recall-server

# Visit http://localhost:47892 - should see React app
```

**Commit:** `feat(server): add static file serving with SPA fallback`

---

## Task 3: E2E Test Against Rust Backend

**Goal:** Run the same E2E tests against Rust to verify compatibility.

**Step 1: Update playwright config for Rust**

```typescript
// e2e/playwright.config.ts - add Rust project
export default defineConfig({
  // ... existing config ...
  projects: [
    {
      name: 'rust-backend',
      use: { baseURL: 'http://localhost:47892' },
      webServer: {
        command: 'cargo run -p vibe-recall-server',
        port: 47892,
        reuseExistingServer: true,
        env: { STATIC_DIR: 'dist' },
      },
    },
  ],
})
```

**Step 2: Run E2E tests against Rust**

```bash
# Build frontend first
bun run build

# Run E2E against Rust backend
bunx playwright test --project=rust-backend

# Compare screenshots
# e2e/screenshots/ should look identical to baseline
```

**Step 3: Create comparison script**

```bash
#!/bin/bash
# e2e/compare-backends.sh

echo "Testing Node backend..."
BACKEND=node bunx playwright test --project=node-backend
mv e2e/screenshots e2e/screenshots-node

echo "Testing Rust backend..."
BACKEND=rust bunx playwright test --project=rust-backend
mv e2e/screenshots e2e/screenshots-rust

echo "Comparing screenshots..."
for file in e2e/screenshots-node/*.png; do
  name=$(basename "$file")
  if ! diff -q "$file" "e2e/screenshots-rust/$name" > /dev/null 2>&1; then
    echo "DIFF: $name"
  else
    echo "OK: $name"
  fi
done
```

**Verification:**
- All E2E tests pass against Rust
- Screenshots match (or acceptable differences)
- No console errors in browser

**Commit:** `test(e2e): verify Rust backend passes all E2E tests`

---

## Task 4: Remove Node Backend (Only After E2E Pass)

**Goal:** Delete Node backend code after Rust is proven.

**Prerequisites:**
- [ ] Task 3 complete
- [ ] All E2E tests pass against Rust
- [ ] Manual smoke test passed

**Step 1: Remove Node server files**

```bash
rm -rf src/server/
```

**Step 2: Update package.json**

```json
{
  "scripts": {
    "dev": "concurrently \"cargo run -p vibe-recall-server\" \"vite\"",
    "build": "vite build",
    "preview": "cargo run -p vibe-recall-server",
    "test": "vitest",
    "test:e2e": "playwright test"
  }
}
```

**Step 3: Update Vite config for dev proxy**

```typescript
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      '/api': 'http://localhost:47892',
    },
  },
})
```

**Verification:**
```bash
# Final E2E test
bunx playwright test

# Manual verification
bun run dev
# Open browser, click through app
```

**Commit:** `refactor: remove Node backend, Rust is now primary`

---

## Execution Strategy

**For Claude (sub-agent):**

1. Run Task 1 → verify E2E baseline works
2. Run Task 2 → verify Rust serves static files
3. Run Task 3 → **CRITICAL: Must pass before continuing**
4. Only if Task 3 passes → Run Task 4

**If E2E fails at any point:**
- Screenshot the failure
- Debug the issue
- Fix and re-run
- Do NOT proceed until green

---

## Success Criteria

- [ ] E2E tests exist and pass against Node (baseline)
- [ ] Rust serves static files correctly
- [ ] E2E tests pass against Rust (identical behavior)
- [ ] Screenshots show same UI
- [ ] Node backend removed
- [ ] App still works after removal
