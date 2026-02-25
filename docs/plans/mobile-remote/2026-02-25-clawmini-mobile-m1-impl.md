# clawmini Mobile M1 — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship an Expo native app that pairs with Mac via QR and shows a real-time live dashboard of AI agent sessions.

**Architecture:** Monorepo restructure (`apps/web`, `apps/mobile`, `packages/shared`), Expo/React Native + NativeWind, keypair auth, dumb relay, ts-rs type sync.

**Tech Stack:** Expo SDK 54, React Native, NativeWind, Turborepo, Bun workspaces, tweetnacl, ts-rs, Axum relay

**Design doc:** `docs/plans/mobile-remote/2026-02-25-clawmini-mobile-m1-design.md`

---

## Phase 1: Monorepo Infrastructure

### Task 1: Move web app to `apps/web/`

**Files:**
- Create: `apps/web/` directory
- Move: `src/` → `apps/web/src/`
- Move: `public/` → `apps/web/public/`
- Move: `index.html` → `apps/web/index.html`
- Move: `vite.config.ts` → `apps/web/vite.config.ts`
- Move: `vitest.config.ts` → `apps/web/vitest.config.ts`
- Move: `tsconfig.json` → `apps/web/tsconfig.json`
- Move: `tsconfig.app.json` → `apps/web/tsconfig.app.json`
- Move: `tsconfig.node.json` → `apps/web/tsconfig.node.json`
- Move: `eslint.config.js` → `apps/web/eslint.config.js`
- Move: `playwright.config.ts` → `apps/web/playwright.config.ts`
- Move: `e2e/` → `apps/web/e2e/`
- Move: `tests/` → `apps/web/tests/`
- Create: `apps/web/package.json` (split from root)
- Modify: root `package.json` (workspace root, remove app-specific deps)
- Create: `tsconfig.base.json` (shared TS config at root)

**Step 1: Create directory structure**

```bash
mkdir -p apps/web
```

**Step 2: Move files with git (preserves history)**

```bash
git mv src apps/web/src
git mv public apps/web/public
git mv index.html apps/web/index.html
git mv vite.config.ts apps/web/vite.config.ts
git mv vitest.config.ts apps/web/vitest.config.ts
git mv tsconfig.json apps/web/tsconfig.json
git mv tsconfig.app.json apps/web/tsconfig.app.json
git mv tsconfig.node.json apps/web/tsconfig.node.json
git mv eslint.config.js apps/web/eslint.config.js
git mv playwright.config.ts apps/web/playwright.config.ts
git mv e2e apps/web/e2e
git mv tests apps/web/tests
```

**Step 3: Create `apps/web/package.json`**

Split all React/frontend dependencies from root `package.json` into `apps/web/package.json`. Keep:
- `name`: `@clawmini/web`
- All React, Radix, Recharts, Tailwind, Vite, Vitest, Playwright deps
- All scripts: `dev:client`, `build`, `test:client`, `test:e2e`, `lint`, `typecheck`

Root `package.json` becomes workspace root with only:
- `name`: `claude-view`
- `workspaces` field
- `scripts` that delegate to workspace packages and Rust
- Shared devDeps: `typescript`, `turbo`

**Step 4: Create `tsconfig.base.json` at root**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "jsx": "react-jsx"
  }
}
```

Update `apps/web/tsconfig.json` to extend: `"extends": "../../tsconfig.base.json"`

**Step 5: Fix path alias in vite.config.ts**

Update `apps/web/vite.config.ts` resolve alias:
```ts
resolve: {
  alias: {
    '@': path.resolve(__dirname, './src'),
  },
},
```

**Step 6: Verify web app builds and tests pass**

```bash
cd apps/web && bun run build
cd apps/web && bun run test:client -- --run
```

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: move web app to apps/web/ (monorepo restructure step 1)"
```

---

### Task 2: Set up Bun workspaces + Turborepo

**Files:**
- Modify: root `package.json` (add workspaces)
- Create: `turbo.json`

**Step 1: Add workspace config to root package.json**

```json
{
  "name": "claude-view",
  "private": true,
  "workspaces": [
    "apps/*",
    "packages/*"
  ],
  "scripts": {
    "dev": "turbo run dev",
    "build": "turbo run build",
    "test": "turbo run test",
    "lint": "turbo run lint",
    "typecheck": "turbo run typecheck",
    "dev:server": "cargo run -p claude-view-server",
    "test:rust": "cargo test --workspace"
  },
  "devDependencies": {
    "turbo": "^2",
    "typescript": "^5.9"
  }
}
```

**Step 2: Create `turbo.json`**

```json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "test": {
      "dependsOn": ["^build"]
    },
    "lint": {},
    "typecheck": {
      "dependsOn": ["^build"]
    }
  }
}
```

**Step 3: Install turbo and regenerate lockfiles**

```bash
bun add -D turbo
bun install
```

**Step 4: Verify workspace resolution**

```bash
bun run build
```

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: add Bun workspaces + Turborepo configuration"
```

---

### Task 3: Create `packages/shared/`

Extract reusable TypeScript logic from `apps/web/src/` into a shared package.

**Files:**
- Create: `packages/shared/package.json`
- Create: `packages/shared/tsconfig.json`
- Create: `packages/shared/src/index.ts`
- Move: `apps/web/src/lib/mobile-crypto.ts` → `packages/shared/src/crypto/nacl.ts`
- Move: `apps/web/src/lib/mobile-storage.ts` → `packages/shared/src/crypto/storage.ts` (refactor to abstract interface)
- Move: `apps/web/src/hooks/use-mobile-relay.ts` → `packages/shared/src/relay/use-mobile-relay.ts`
- Create: `packages/shared/src/utils/format-cost.ts`
- Create: `packages/shared/src/utils/group-sessions.ts`
- Create: `packages/shared/src/utils/format-duration.ts`
- Create: `packages/shared/src/types/` (placeholder for ts-rs output)

**Step 1: Create package structure**

```bash
mkdir -p packages/shared/src/{crypto,relay,utils,types}
```

**Step 2: Create `packages/shared/package.json`**

```json
{
  "name": "@clawmini/shared",
  "version": "0.0.1",
  "private": true,
  "type": "module",
  "main": "./src/index.ts",
  "types": "./src/index.ts",
  "dependencies": {
    "tweetnacl": "^1.0.3",
    "tweetnacl-util": "^0.15.1"
  }
}
```

**Step 3: Create `packages/shared/tsconfig.json`**

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "outDir": "./dist",
    "rootDir": "./src"
  },
  "include": ["src/**/*"]
}
```

**Step 4: Extract crypto module**

Move `apps/web/src/lib/mobile-crypto.ts` → `packages/shared/src/crypto/nacl.ts`.

Create `packages/shared/src/crypto/storage.ts` with an abstract interface:

```ts
export interface KeyStorage {
  getItem(key: string): Promise<string | null>;
  setItem(key: string, value: string): Promise<void>;
  removeItem(key: string): Promise<void>;
}
```

The web app passes an IndexedDB implementation. The Expo app will pass an `expo-secure-store` implementation. Crypto logic doesn't care.

**Step 5: Extract relay hook**

Move `apps/web/src/hooks/use-mobile-relay.ts` → `packages/shared/src/relay/use-mobile-relay.ts`.

Parameterize the storage backend (takes `KeyStorage` interface instead of importing IndexedDB directly).

**Step 6: Extract utility functions**

Create these in `packages/shared/src/utils/`:

- `format-cost.ts` — `formatUsd(cents: number): string`
- `group-sessions.ts` — `groupByAgentState(sessions: LiveSession[]): { needsYou: LiveSession[], autonomous: LiveSession[] }`
- `format-duration.ts` — `formatDuration(seconds: number): string`

Extract from existing code in `apps/web/src/lib/` and `apps/web/src/components/`.

**Step 7: Create barrel export**

```ts
// packages/shared/src/index.ts
export * from './crypto/nacl';
export * from './crypto/storage';
export * from './relay/use-mobile-relay';
export * from './utils/format-cost';
export * from './utils/group-sessions';
export * from './utils/format-duration';
export type * from './types';
```

**Step 8: Update web app imports**

Replace all `apps/web/src/` imports of moved modules with `@clawmini/shared`:

```ts
// Before
import { encryptForDevice } from '../lib/mobile-crypto';
// After
import { encryptForDevice } from '@clawmini/shared';
```

**Step 9: Verify web app still builds**

```bash
cd apps/web && bun run build && bun run test:client -- --run
```

**Step 10: Commit**

```bash
git add -A
git commit -m "feat: create packages/shared with extracted crypto, relay, and utils"
```

---

### Task 4: Wire up ts-rs type generation

**Files:**
- Modify: `crates/core/Cargo.toml` (add ts-rs dependency)
- Modify: `crates/server/Cargo.toml` (add ts-rs dependency)
- Modify: `crates/server/src/live/types.rs` (add `#[derive(TS)]` to LiveSession and related structs)
- Create: `packages/shared/src/types/generated/` (output directory)
- Create: `scripts/generate-types.sh`

**Step 1: Add ts-rs to crate dependencies**

In `crates/server/Cargo.toml` under `[dependencies]`:
```toml
ts-rs = { workspace = true }
```

(`ts-rs` is already in workspace `Cargo.toml` as `ts-rs = { version = "11", features = ["serde-compat"] }`)

**Step 2: Add `#[derive(TS)]` to key structs**

Find and annotate these structs (in `crates/server/src/live/` or `crates/core/src/`):

```rust
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export, export_to = "../../packages/shared/src/types/generated/")]
pub struct LiveSession { ... }

// Repeat for: AgentState, CostBreakdown, TokenUsage, SubAgentInfo, ProgressItem, ToolUsed, SessionEvent
```

**Step 3: Create generation script**

```bash
#!/bin/bash
# scripts/generate-types.sh
set -euo pipefail
echo "Generating TypeScript types from Rust structs..."
cargo test -p claude-view-server export_bindings -- --nocapture 2>/dev/null || true
echo "Types written to packages/shared/src/types/generated/"
ls packages/shared/src/types/generated/*.ts 2>/dev/null | head -20
```

**Step 4: Run type generation**

```bash
chmod +x scripts/generate-types.sh
./scripts/generate-types.sh
```

**Step 5: Create types barrel export**

```ts
// packages/shared/src/types/index.ts
export * from './generated/LiveSession';
export * from './generated/AgentState';
// ... etc
```

**Step 6: Verify types match existing TS types in web app**

Compare generated types with `apps/web/src/types/` to ensure compatibility. Fix any mismatches.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: wire up ts-rs for Rust→TypeScript type generation"
```

---

## Phase 2: Relay Fixes

### Task 5: Fix relay pairing bugs

The 3 known bugs from the old Phase A plan, plus the `pair_complete` handler. These are absorbed into one task since they're all small changes in the same files.

**Files:**
- Modify: `crates/relay/src/pairing.rs` (add x25519_pubkey to ClaimRequest, forward in pair_complete)
- Modify: `crates/server/src/live/relay_client.rs` (always connect, handle pair_complete)
- Test: `crates/relay/tests/`

**Step 1: Write failing test for x25519_pubkey in ClaimRequest**

Create test in `crates/relay/tests/pairing_test.rs`:

```rust
#[tokio::test]
async fn test_claim_includes_x25519_pubkey() {
    // Setup: create offer, then claim with x25519_pubkey
    // Assert: pair_complete message sent to Mac includes x25519_pubkey
}
```

Run: `cargo test -p claude-view-relay pairing_test -- --nocapture`
Expected: FAIL

**Step 2: Fix ClaimRequest struct**

In `crates/relay/src/pairing.rs`, add field to `ClaimRequest`:

```rust
#[derive(Deserialize)]
pub struct ClaimRequest {
    pub one_time_token: String,
    pub device_id: String,
    pub pubkey: String,
    pub pubkey_encrypted_blob: Option<String>,
    pub x25519_pubkey: Option<String>,  // ADD THIS
}
```

In the claim handler, include `x25519_pubkey` in the `pair_complete` JSON sent to Mac:

```rust
let pair_complete = serde_json::json!({
    "type": "pair_complete",
    "device_id": &claim.device_id,
    "pubkey": &claim.pubkey,
    "pubkey_encrypted_blob": &claim.pubkey_encrypted_blob,
    "x25519_pubkey": &claim.x25519_pubkey,  // ADD THIS
});
```

**Step 3: Run test to verify it passes**

```bash
cargo test -p claude-view-relay pairing_test -- --nocapture
```

**Step 4: Fix relay_client always-connect bug**

In `crates/server/src/live/relay_client.rs`, remove the early return when no paired devices exist. Instead, connect to relay always (to receive `pair_complete` messages), but only send session data when paired devices exist.

```rust
// BEFORE (chicken-and-egg):
let paired = load_paired_devices()?;
if paired.is_empty() {
    tokio::time::sleep(Duration::from_secs(10)).await;
    continue;
}

// AFTER:
// Always connect. Check paired devices only when sending data.
let paired = load_paired_devices().unwrap_or_default();
// ... connect to relay ...
// Only send session snapshots if paired_devices is non-empty
if !paired.is_empty() {
    send_snapshot(&ws_tx, &live_sessions, &paired, &identity).await;
}
```

**Step 5: Implement pair_complete handler**

In `crates/server/src/live/relay_client.rs`, replace the TODO stub in the incoming message handler:

```rust
if msg_type == "pair_complete" {
    let device_id = json["device_id"].as_str().unwrap_or_default();
    let x25519_pubkey = json["x25519_pubkey"].as_str().unwrap_or_default();
    let name = format!("Phone ({})", &device_id[..8.min(device_id.len())]);

    if let Err(e) = add_paired_device(device_id, x25519_pubkey, &name) {
        tracing::error!("Failed to store paired device: {e}");
    } else {
        tracing::info!("Paired with device: {device_id}");
        // Reload paired devices to start streaming
        paired_devices = load_paired_devices().unwrap_or_default();
    }
}
```

**Step 6: Run all relay tests**

```bash
cargo test -p claude-view-relay -- --nocapture
cargo test -p claude-view-server relay -- --nocapture
```

**Step 7: Commit**

```bash
git add -A
git commit -m "fix: relay pairing bugs — x25519_pubkey forwarding, always-connect, pair_complete handler"
```

---

## Phase 3: Expo App

### Task 6: Scaffold Expo app

**Files:**
- Create: `apps/mobile/` (entire Expo project)

**Step 1: Create Expo project**

```bash
cd apps
npx create-expo-app mobile --template blank-typescript
cd mobile
```

**Step 2: Install dependencies**

```bash
npx expo install expo-router expo-camera expo-secure-store expo-notifications expo-haptics
npx expo install nativewind tailwindcss react-native-reanimated @gorhom/bottom-sheet
npx expo install react-native-gesture-handler react-native-safe-area-context
npx expo install tweetnacl tweetnacl-util
bun add -D @storybook/react-native
```

**Step 3: Create `app.config.ts` with 3 build variants**

```ts
import { ExpoConfig, ConfigContext } from 'expo/config';

const IS_DEV = process.env.APP_VARIANT === 'development';
const IS_PREVIEW = process.env.APP_VARIANT === 'preview';

const getBundleId = () => {
  if (IS_DEV) return 'com.clawmini.dev';
  if (IS_PREVIEW) return 'com.clawmini.preview';
  return 'com.clawmini.app';
};

const getAppName = () => {
  if (IS_DEV) return 'clawmini (dev)';
  if (IS_PREVIEW) return 'clawmini (preview)';
  return 'clawmini';
};

export default ({ config }: ConfigContext): ExpoConfig => ({
  ...config,
  name: getAppName(),
  slug: 'clawmini',
  version: '0.1.0',
  scheme: 'claude-view',
  orientation: 'portrait',
  icon: './assets/icon.png',
  splash: { image: './assets/splash.png', resizeMode: 'contain', backgroundColor: '#0F172A' },
  ios: {
    bundleIdentifier: getBundleId(),
    supportsTablet: false,
    associatedDomains: IS_DEV || IS_PREVIEW ? [] : ['applinks:m.claudeview.ai'],
  },
  android: {
    package: getBundleId(),
    adaptiveIcon: { foregroundImage: './assets/adaptive-icon.png', backgroundColor: '#0F172A' },
    intentFilters: IS_DEV || IS_PREVIEW ? [] : [{
      action: 'VIEW',
      autoVerify: true,
      data: [{ scheme: 'https', host: 'm.claudeview.ai', pathPrefix: '/' }],
      category: ['BROWSABLE', 'DEFAULT'],
    }],
  },
  plugins: [
    'expo-router',
    ['expo-camera', { cameraPermission: 'Allow clawmini to scan QR codes for pairing.' }],
    'expo-secure-store',
    'expo-notifications',
  ],
});
```

**Step 4: Set up NativeWind**

Create `apps/mobile/tailwind.config.ts`:

```ts
import type { Config } from 'tailwindcss';

export default {
  content: ['./app/**/*.{ts,tsx}', './components/**/*.{ts,tsx}'],
  presets: [require('nativewind/preset')],
  theme: {
    extend: {
      colors: {
        base: '#0F172A',
        surface: '#1E293B',
        border: '#334155',
        muted: '#94A3B8',
        'status-green': '#22C55E',
        'status-amber': '#F59E0B',
        'status-red': '#EF4444',
        accent: '#6366F1',
      },
      fontFamily: {
        mono: ['FiraCode'],
        sans: ['FiraSans'],
      },
    },
  },
  plugins: [],
} satisfies Config;
```

Create `apps/mobile/global.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

**Step 5: Set up Expo Router file structure**

```bash
mkdir -p apps/mobile/app
mkdir -p apps/mobile/components
mkdir -p apps/mobile/hooks
mkdir -p apps/mobile/lib
mkdir -p apps/mobile/assets
```

Create `apps/mobile/app/_layout.tsx`:

```tsx
import '../global.css';
import { Stack } from 'expo-router';

export default function RootLayout() {
  return (
    <Stack screenOptions={{ headerShown: false }}>
      <Stack.Screen name="index" />
      <Stack.Screen name="dashboard" />
    </Stack>
  );
}
```

Create `apps/mobile/app/index.tsx` (entry — routes to pair or dashboard):

```tsx
import { Redirect } from 'expo-router';
import { usePairingStatus } from '../hooks/use-pairing-status';

export default function Index() {
  const { isPaired } = usePairingStatus();
  return <Redirect href={isPaired ? '/dashboard' : '/pair'} />;
}
```

**Step 6: Add workspace dependency on shared package**

In `apps/mobile/package.json`:

```json
{
  "dependencies": {
    "@clawmini/shared": "workspace:*"
  }
}
```

**Step 7: Verify Expo starts**

```bash
cd apps/mobile && npx expo start
```

**Step 8: Commit**

```bash
git add -A
git commit -m "feat: scaffold Expo app with NativeWind, Expo Router, 3 build variants"
```

---

### Task 7: Pair screen

**Files:**
- Create: `apps/mobile/app/pair.tsx`
- Create: `apps/mobile/hooks/use-pairing-status.ts`
- Create: `apps/mobile/lib/secure-store-adapter.ts`

**Step 1: Create SecureStore adapter implementing KeyStorage interface**

```ts
// apps/mobile/lib/secure-store-adapter.ts
import * as SecureStore from 'expo-secure-store';
import type { KeyStorage } from '@clawmini/shared';

export const secureStoreAdapter: KeyStorage = {
  async getItem(key: string) {
    return SecureStore.getItemAsync(key);
  },
  async setItem(key: string, value: string) {
    await SecureStore.setItemAsync(key, value);
  },
  async removeItem(key: string) {
    await SecureStore.deleteItemAsync(key);
  },
};
```

**Step 2: Create pairing status hook**

```ts
// apps/mobile/hooks/use-pairing-status.ts
import { useState, useEffect } from 'react';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export function usePairingStatus() {
  const [isPaired, setIsPaired] = useState<boolean | null>(null);

  useEffect(() => {
    secureStoreAdapter.getItem('relay_url').then((url) => {
      setIsPaired(url !== null);
    });
  }, []);

  return { isPaired, refresh: () => { /* re-check */ } };
}
```

**Step 3: Build Pair screen**

```tsx
// apps/mobile/app/pair.tsx
import { useState } from 'react';
import { View, Text } from 'react-native';
import { CameraView, useCameraPermissions } from 'expo-camera';
import * as Haptics from 'expo-haptics';
import { router } from 'expo-router';
import { generatePhoneKeys, claimPairing } from '@clawmini/shared';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export default function PairScreen() {
  const [permission, requestPermission] = useCameraPermissions();
  const [scanned, setScanned] = useState(false);

  const handleBarCodeScanned = async ({ data }: { data: string }) => {
    if (scanned) return;
    setScanned(true);
    await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);

    try {
      // Parse QR payload (URL with k, t, r params)
      const url = new URL(data);
      const macPubkey = url.searchParams.get('k');
      const token = url.searchParams.get('t');
      const relayUrl = url.searchParams.get('r');

      if (!macPubkey || !token || !relayUrl) throw new Error('Invalid QR');

      // Generate phone keypair
      const keys = await generatePhoneKeys();

      // Claim pairing via relay
      await claimPairing({ macPubkey, token, relayUrl, keys, storage: secureStoreAdapter });

      router.replace('/dashboard');
    } catch (e) {
      setScanned(false);
      // Show error toast
    }
  };

  if (!permission?.granted) {
    return (
      <View className="flex-1 bg-base items-center justify-center px-8">
        <Text className="text-white text-lg text-center mb-6">
          Camera access needed to scan QR code
        </Text>
        <Text className="text-accent text-lg" onPress={requestPermission}>
          Grant Access
        </Text>
      </View>
    );
  }

  return (
    <View className="flex-1 bg-base">
      <CameraView
        className="flex-1"
        barcodeScannerSettings={{ barcodeTypes: ['qr'] }}
        onBarcodeScanned={scanned ? undefined : handleBarCodeScanned}
      />
      <View className="absolute bottom-0 left-0 right-0 p-8 items-center">
        <Text className="text-white text-lg text-center">
          Scan the QR code from your Mac's claude-view
        </Text>
        <Text className="text-muted text-sm mt-2 text-center">
          One scan. No account. No password. Ever.
        </Text>
      </View>
    </View>
  );
}
```

**Step 4: Test on iOS simulator**

```bash
cd apps/mobile && npx expo run:ios
```

Verify: camera opens, can scan a QR code (use a test QR from Mac).

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: pair screen — QR scan with expo-camera, SecureStore keypair storage"
```

---

### Task 8: Dashboard screen

**Files:**
- Create: `apps/mobile/app/dashboard.tsx`
- Create: `apps/mobile/components/SessionCard.tsx`
- Create: `apps/mobile/components/SummaryBar.tsx`
- Create: `apps/mobile/components/ConnectionStatus.tsx`
- Create: `apps/mobile/hooks/use-relay-sessions.ts`

**Step 1: Create relay sessions hook**

Wraps `@clawmini/shared`'s `useMobileRelay` with `secureStoreAdapter`:

```ts
// apps/mobile/hooks/use-relay-sessions.ts
import { useMobileRelay } from '@clawmini/shared';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

export function useRelaySessions() {
  return useMobileRelay({ storage: secureStoreAdapter });
}
```

Returns: `{ sessions, connectionState, disconnect }`

**Step 2: Create SessionCard component**

```tsx
// apps/mobile/components/SessionCard.tsx
import { View, Text, Pressable } from 'react-native';
import { formatUsd, type LiveSession } from '@clawmini/shared';

interface Props {
  session: LiveSession;
  onPress: () => void;
}

export function SessionCard({ session, onPress }: Props) {
  const contextPct = session.contextWindowTokens > 0
    ? Math.round((session.contextWindowTokens / 200000) * 100)
    : 0;

  return (
    <Pressable
      className="bg-surface rounded-lg p-4 mb-2 active:opacity-80"
      onPress={onPress}
    >
      <Text className="text-white font-sans font-semibold text-base">
        {session.projectDisplayName}
      </Text>
      <Text className="text-muted text-sm mt-1">
        {session.agentState?.label ?? session.status}
      </Text>
      <View className="flex-row justify-between items-center mt-3">
        <Text className="text-muted font-mono text-sm">
          {formatUsd(session.cost?.totalUsd ?? 0)}
        </Text>
        <View className="flex-row items-center">
          <View className="w-24 h-2 bg-border rounded-full overflow-hidden">
            <View
              className="h-full bg-accent rounded-full"
              style={{ width: `${contextPct}%` }}
            />
          </View>
          <Text className="text-muted font-mono text-xs ml-2">{contextPct}%</Text>
        </View>
      </View>
    </Pressable>
  );
}
```

**Step 3: Create SummaryBar component**

```tsx
// apps/mobile/components/SummaryBar.tsx
import { View, Text } from 'react-native';
import { formatUsd, type LiveSession } from '@clawmini/shared';

export function SummaryBar({ sessions }: { sessions: LiveSession[] }) {
  const needsYou = sessions.filter(s => s.agentState?.group === 'needs_you').length;
  const autonomous = sessions.filter(s => s.agentState?.group === 'autonomous').length;
  const totalCost = sessions.reduce((sum, s) => sum + (s.cost?.totalUsd ?? 0), 0);

  return (
    <View className="bg-surface border-t border-border px-4 py-3 flex-row justify-between">
      <Text className="text-status-amber font-sans text-sm">{needsYou} needs you</Text>
      <Text className="text-status-green font-sans text-sm">{autonomous} auto</Text>
      <Text className="text-muted font-mono text-sm">{formatUsd(totalCost)}</Text>
    </View>
  );
}
```

**Step 4: Create ConnectionStatus component**

```tsx
// apps/mobile/components/ConnectionStatus.tsx
import { View, Text } from 'react-native';

type State = 'connected' | 'connecting' | 'disconnected';

export function ConnectionStatus({ state }: { state: State }) {
  const color = state === 'connected' ? 'bg-status-green'
    : state === 'connecting' ? 'bg-status-amber'
    : 'bg-status-red';

  const label = state === 'connected' ? 'Connected'
    : state === 'connecting' ? 'Connecting'
    : 'Mac offline';

  return (
    <View className="flex-row items-center">
      <View className={`w-2 h-2 rounded-full ${color} mr-2`} />
      <Text className="text-muted text-sm">{label}</Text>
    </View>
  );
}
```

**Step 5: Build Dashboard screen**

```tsx
// apps/mobile/app/dashboard.tsx
import { View, Text, ScrollView, RefreshControl } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { useState, useCallback } from 'react';
import { groupByAgentState } from '@clawmini/shared';
import { useRelaySessions } from '../hooks/use-relay-sessions';
import { SessionCard } from '../components/SessionCard';
import { SummaryBar } from '../components/SummaryBar';
import { ConnectionStatus } from '../components/ConnectionStatus';

export default function DashboardScreen() {
  const { sessions, connectionState } = useRelaySessions();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { needsYou, autonomous } = groupByAgentState(Object.values(sessions));

  return (
    <SafeAreaView className="flex-1 bg-base" edges={['top']}>
      {/* Header */}
      <View className="flex-row justify-between items-center px-4 py-3">
        <Text className="text-white font-sans font-bold text-xl">clawmini</Text>
        <ConnectionStatus state={connectionState} />
      </View>

      {/* Session list */}
      <ScrollView className="flex-1 px-4">
        {needsYou.length > 0 && (
          <View className="mb-4">
            <Text className="text-status-amber font-sans font-semibold text-sm mb-2 uppercase tracking-wider">
              Needs You
            </Text>
            {needsYou.map(s => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </View>
        )}

        {autonomous.length > 0 && (
          <View className="mb-4">
            <Text className="text-status-green font-sans font-semibold text-sm mb-2 uppercase tracking-wider">
              Autonomous
            </Text>
            {autonomous.map(s => (
              <SessionCard key={s.id} session={s} onPress={() => setSelectedId(s.id)} />
            ))}
          </View>
        )}

        {needsYou.length === 0 && autonomous.length === 0 && (
          <View className="flex-1 items-center justify-center py-20">
            <Text className="text-muted text-lg">
              {connectionState === 'disconnected' ? 'Mac offline' : 'No active sessions'}
            </Text>
          </View>
        )}
      </ScrollView>

      {/* Summary bar */}
      <SummaryBar sessions={Object.values(sessions)} />

      {/* Bottom sheet will be added in Task 9 */}
    </SafeAreaView>
  );
}
```

**Step 6: Test with live Mac data**

Start Mac dev server with `RELAY_URL` set, scan QR from Expo Go, verify sessions appear.

**Step 7: Commit**

```bash
git add -A
git commit -m "feat: dashboard screen — session cards grouped by agent state, summary bar"
```

---

### Task 9: Session detail bottom sheet

**Files:**
- Create: `apps/mobile/components/SessionDetailSheet.tsx`
- Modify: `apps/mobile/app/dashboard.tsx` (add bottom sheet)

**Step 1: Create SessionDetailSheet**

```tsx
// apps/mobile/components/SessionDetailSheet.tsx
import { View, Text, ScrollView } from 'react-native';
import BottomSheet, { BottomSheetScrollView } from '@gorhom/bottom-sheet';
import { forwardRef, useMemo } from 'react';
import { formatUsd, formatDuration, type LiveSession } from '@clawmini/shared';

interface Props {
  session: LiveSession | null;
}

export const SessionDetailSheet = forwardRef<BottomSheet, Props>(({ session }, ref) => {
  const snapPoints = useMemo(() => ['50%', '90%'], []);

  if (!session) return null;

  const contextPct = session.contextWindowTokens > 0
    ? Math.round((session.contextWindowTokens / 200000) * 100)
    : 0;

  return (
    <BottomSheet
      ref={ref}
      index={-1}
      snapPoints={snapPoints}
      enablePanDownToClose
      backgroundStyle={{ backgroundColor: '#1E293B' }}
      handleIndicatorStyle={{ backgroundColor: '#94A3B8' }}
    >
      <BottomSheetScrollView className="px-4 pb-8">
        {/* Header */}
        <Text className="text-white font-sans font-bold text-xl">
          {session.projectDisplayName}
        </Text>
        <Text className="text-muted text-sm mt-1">{session.projectPath}</Text>
        {session.gitBranch && (
          <Text className="text-accent text-sm mt-1">branch: {session.gitBranch}</Text>
        )}

        {/* Status row */}
        <View className="flex-row flex-wrap mt-4 gap-4">
          <InfoItem label="Status" value={session.agentState?.label ?? session.status} />
          <InfoItem label="Model" value={session.model ?? 'unknown'} />
          <InfoItem label="Turns" value={String(session.turnCount)} />
          {session.startedAt && session.startedAt > 0 && (
            <InfoItem
              label="Time"
              value={formatDuration(Math.floor(Date.now() / 1000) - session.startedAt)}
            />
          )}
        </View>

        {/* Cost breakdown */}
        <SectionHeader title="Cost" />
        <View className="bg-base rounded-lg p-3">
          <CostRow label="Input" value={session.cost?.inputUsd ?? 0} />
          <CostRow label="Output" value={session.cost?.outputUsd ?? 0} />
          <CostRow label="Total" value={session.cost?.totalUsd ?? 0} bold />
        </View>

        {/* Context */}
        <SectionHeader title="Context" />
        <View className="bg-base rounded-lg p-3">
          <View className="w-full h-3 bg-border rounded-full overflow-hidden">
            <View className="h-full bg-accent rounded-full" style={{ width: `${contextPct}%` }} />
          </View>
          <Text className="text-muted font-mono text-xs mt-2">
            {Math.round(session.contextWindowTokens / 1000)}k / 200k tokens ({contextPct}%)
          </Text>
        </View>

        {/* Activity */}
        {session.currentActivity && (
          <>
            <SectionHeader title="Activity" />
            <Text className="text-white text-sm">{session.currentActivity}</Text>
          </>
        )}

        {/* Sub-agents */}
        {session.subAgents && session.subAgents.length > 0 && (
          <>
            <SectionHeader title={`Sub-agents (${session.subAgents.length})`} />
            {session.subAgents.map((a, i) => (
              <View key={i} className="flex-row items-center py-1">
                <Text className="text-status-green text-sm mr-2">
                  {a.status === 'done' ? '✓' : '⚡'}
                </Text>
                <Text className="text-white text-sm">{a.name}</Text>
                <Text className="text-muted text-sm ml-2">{a.status}</Text>
              </View>
            ))}
          </>
        )}

        {/* Progress items */}
        {session.progressItems && session.progressItems.length > 0 && (
          <>
            <SectionHeader title="Progress" />
            {session.progressItems.map((item, i) => (
              <View key={i} className="flex-row items-center py-1">
                <Text className="text-sm mr-2">
                  {item.status === 'completed' ? '✓' : '○'}
                </Text>
                <Text className={`text-sm ${item.status === 'completed' ? 'text-muted' : 'text-white'}`}>
                  {item.subject}
                </Text>
              </View>
            ))}
          </>
        )}

        {/* M1.5 teaser */}
        <View className="mt-6 bg-base rounded-lg p-4 items-center opacity-50">
          <Text className="text-muted text-sm">Approve / Deny — coming in M1.5</Text>
        </View>
      </BottomSheetScrollView>
    </BottomSheet>
  );
});

function SectionHeader({ title }: { title: string }) {
  return <Text className="text-muted font-sans text-xs uppercase tracking-wider mt-4 mb-2">{title}</Text>;
}

function InfoItem({ label, value }: { label: string; value: string }) {
  return (
    <View>
      <Text className="text-muted text-xs">{label}</Text>
      <Text className="text-white text-sm font-sans">{value}</Text>
    </View>
  );
}

function CostRow({ label, value, bold }: { label: string; value: number; bold?: boolean }) {
  return (
    <View className="flex-row justify-between py-1">
      <Text className={`text-sm ${bold ? 'text-white font-semibold' : 'text-muted'}`}>{label}</Text>
      <Text className={`font-mono text-sm ${bold ? 'text-white font-semibold' : 'text-muted'}`}>
        {formatUsd(value)}
      </Text>
    </View>
  );
}
```

**Step 2: Wire bottom sheet into dashboard**

Add to `apps/mobile/app/dashboard.tsx`:

```tsx
import { useRef } from 'react';
import BottomSheet from '@gorhom/bottom-sheet';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { SessionDetailSheet } from '../components/SessionDetailSheet';

// Inside DashboardScreen:
const bottomSheetRef = useRef<BottomSheet>(null);
const selectedSession = selectedId ? sessions[selectedId] : null;

// On card press:
onPress={() => {
  setSelectedId(s.id);
  bottomSheetRef.current?.snapToIndex(0);
}}

// Wrap entire return in GestureHandlerRootView, add sheet at bottom:
<SessionDetailSheet ref={bottomSheetRef} session={selectedSession} />
```

**Step 3: Test interaction**

Verify: tap card → sheet slides up half screen → drag up for full → drag down to dismiss.

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: session detail bottom sheet with cost, context, sub-agents, progress"
```

---

### Task 10: Push notifications

**Files:**
- Create: `apps/mobile/hooks/use-push-notifications.ts`
- Modify: `crates/relay/src/lib.rs` (add push token route)
- Create: `crates/relay/src/push.rs`
- Modify: `apps/mobile/app/_layout.tsx` (register on startup)

**Step 1: Create push token registration on relay**

```rust
// crates/relay/src/push.rs
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use std::sync::Arc;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct RegisterToken {
    pub device_id: String,
    pub token: String, // Expo push token
}

pub async fn register_push_token(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterToken>,
) -> StatusCode {
    state.push_tokens.insert(body.device_id, body.token);
    StatusCode::OK
}
```

Add to `AppState`: `pub push_tokens: DashMap<String, String>`
Add route: `POST /push-tokens`

**Step 2: Create push notification hook in Expo app**

```ts
// apps/mobile/hooks/use-push-notifications.ts
import { useEffect, useRef } from 'react';
import * as Notifications from 'expo-notifications';
import { Platform } from 'react-native';
import { secureStoreAdapter } from '../lib/secure-store-adapter';

Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowAlert: true,
    shouldPlaySound: true,
    shouldSetBadge: true,
  }),
});

export function usePushNotifications() {
  const notificationListener = useRef<Notifications.EventSubscription>();

  useEffect(() => {
    registerForPushNotifications();

    notificationListener.current = Notifications.addNotificationResponseReceivedListener(
      (response) => {
        const sessionId = response.notification.request.content.data?.sessionId;
        // Navigate to session detail
      }
    );

    return () => {
      notificationListener.current?.remove();
    };
  }, []);
}

async function registerForPushNotifications() {
  const { status } = await Notifications.requestPermissionsAsync();
  if (status !== 'granted') return;

  const token = (await Notifications.getExpoPushTokenAsync()).data;
  const deviceId = await secureStoreAdapter.getItem('device_id');
  const relayUrl = await secureStoreAdapter.getItem('relay_url');

  if (!deviceId || !relayUrl) return;

  // Register token with relay
  const httpUrl = relayUrl.replace('wss://', 'https://').replace('/ws', '');
  await fetch(`${httpUrl}/push-tokens`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ device_id: deviceId, token }),
  });
}
```

**Step 3: Add push sending logic to relay**

When relay forwards a session update where `agentState.group` transitions to `"needs_you"`, also send an Expo push notification to the registered token.

**Step 4: Register in app layout**

Add to `apps/mobile/app/_layout.tsx`:

```tsx
import { usePushNotifications } from '../hooks/use-push-notifications';

export default function RootLayout() {
  usePushNotifications();
  // ... rest
}
```

**Step 5: Test push notification**

Trigger a session state change on Mac → verify notification appears on phone.

**Step 6: Commit**

```bash
git add -A
git commit -m "feat: push notifications for agent state changes via expo-notifications"
```

---

## Phase 4: Polish & Ship

### Task 11: Landing page

**Files:**
- Create: `apps/landing/index.html`
- Create: `apps/landing/.well-known/apple-app-site-association`
- Create: `apps/landing/_redirects`
- Create: `apps/landing/package.json`

**Step 1: Create landing page**

```html
<!-- apps/landing/index.html -->
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>clawmini — Your AI agents, in your pocket</title>
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { background: #0F172A; color: #F8FAFC; font-family: -apple-system, sans-serif; }
    .container { max-width: 600px; margin: 0 auto; padding: 80px 24px; text-align: center; }
    h1 { font-size: 32px; font-weight: 700; margin-bottom: 16px; }
    p { color: #94A3B8; font-size: 18px; margin-bottom: 48px; }
    .badges { display: flex; gap: 16px; justify-content: center; }
    .badges img { height: 48px; }
  </style>
</head>
<body>
  <div class="container">
    <h1>clawmini</h1>
    <p>Your AI agents, in your pocket. Monitor and control your AI coding sessions from your phone.</p>
    <div class="badges">
      <a href="https://apps.apple.com/app/clawmini/idXXXXXXXXX">
        <img src="https://developer.apple.com/assets/elements/badges/download-on-the-app-store.svg" alt="Download on App Store">
      </a>
      <a href="https://play.google.com/store/apps/details?id=com.clawmini.app">
        <img src="https://play.google.com/intl/en_us/badges/static/images/badges/en_badge_web_generic.png" alt="Get it on Google Play" style="height:48px;">
      </a>
    </div>
  </div>
</body>
</html>
```

**Step 2: Create Apple App Site Association**

```json
{
  "applinks": {
    "details": [
      {
        "appIDs": ["TEAMID.com.clawmini.app"],
        "components": [{ "/": "*" }]
      }
    ]
  }
}
```

**Step 3: Create Cloudflare Pages config**

```
# apps/landing/_redirects
/pair/* /index.html 200
```

**Step 4: Deploy to Cloudflare Pages**

```bash
cd apps/landing && npx wrangler pages deploy . --project-name=clawmini-landing
```

Configure DNS: `m.claudeview.ai` CNAME → Cloudflare Pages.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat: landing page with App Store badges and universal link handler"
```

---

### Task 12: TestFlight build + submission

**Step 1: Configure EAS Build**

```bash
cd apps/mobile && eas build:configure
```

Create `apps/mobile/eas.json`:

```json
{
  "build": {
    "development": {
      "developmentClient": true,
      "distribution": "internal",
      "env": { "APP_VARIANT": "development" }
    },
    "preview": {
      "distribution": "internal",
      "env": { "APP_VARIANT": "preview" }
    },
    "production": {
      "env": { "APP_VARIANT": "production" }
    }
  },
  "submit": {
    "production": {
      "ios": { "appleId": "YOUR_APPLE_ID", "ascAppId": "YOUR_ASC_APP_ID" }
    }
  }
}
```

**Step 2: Build for iOS**

```bash
cd apps/mobile && eas build --platform ios --profile production
```

**Step 3: Submit to TestFlight**

```bash
cd apps/mobile && eas submit --platform ios --profile production
```

**Step 4: Verify on device**

Install from TestFlight. Full E2E: scan QR → sessions appear → push notification fires.

**Step 5: Commit (any config changes)**

```bash
git add -A
git commit -m "chore: EAS build configuration for TestFlight submission"
```

---

## Task Dependency Graph

```
Task 1 (move web) → Task 2 (workspaces) → Task 3 (shared pkg) → Task 4 (ts-rs)
                                                                      ↓
Task 5 (relay fixes) ──────────────────────────────────────→ Task 7 (pair screen)
                                                                      ↓
                                               Task 6 (scaffold) → Task 8 (dashboard)
                                                                      ↓
                                                                Task 9 (detail sheet)
                                                                      ↓
                                                                Task 10 (push)
                                                                      ↓
                                               Task 11 (landing) + Task 12 (TestFlight)
```

Tasks 5 and 6 can run in parallel after Task 4 completes.
Tasks 11 can run in parallel with Tasks 8-10.

## Success Criteria

Same as design doc:
1. Scan QR on Mac → phone shows all active sessions within 2 seconds
2. Session state changes on Mac → phone updates within 1 second
3. Push notification fires when agent state → needs_you
4. "Mac offline" shows correctly when Mac sleeps
5. App is on TestFlight (iOS)
