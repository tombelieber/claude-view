---
status: pending
date: 2026-02-05
purpose: Theme 4 Phase 3 — System Page for Power Users
depends_on: [phase1-foundation]
parallelizable_with: [phase2-classification, phase4-pattern-engine]
---

# Phase 3: System Page

> **Goal:** Create a `/system` page for power users to monitor system health, manage indexing, and control classification.

## Overview

The System Page provides a single location for power users to:
1. Monitor storage usage and performance metrics
2. View data health status (sessions, commits, errors)
3. Manage AI classification (trigger runs, configure provider)
4. Review index history
5. Perform maintenance actions (re-index, clear cache, export, reset)
6. Check Claude CLI installation status

This page is distinct from `/settings` which handles user preferences. `/system` is about the system's operational state and health.

---

## Dependencies

| Dependency | What We Need | Status |
|------------|--------------|--------|
| Phase 1 Foundation | `index_runs` table, `classification_jobs` table | `pending` |
| Existing | `index_metadata` table | Available |
| Existing | Sessions/commits DB | Available |

---

## Tasks

### 3.1 GET /api/system endpoint

**Description:** New API endpoint that aggregates all system health data into a single response.

**File Changes:**

| File | Change |
|------|--------|
| `crates/server/src/routes/mod.rs` | Add `pub mod system;` and nest route |
| `crates/server/src/routes/system.rs` | New file - system endpoint implementation |
| `crates/db/src/queries.rs` | Add `get_storage_stats()`, `get_health_stats()` functions |
| `crates/db/src/lib.rs` | Re-export new query types |

**Subtasks:**

- [ ] 3.1.1 Create `SystemResponse` struct with all nested types
- [ ] 3.1.2 Implement storage calculation (JSONL bytes, index bytes, DB bytes, cache bytes)
- [ ] 3.1.3 Implement performance metrics (last index duration, throughput, sessions/sec)
- [ ] 3.1.4 Implement health stats (counts, error count, status enum)
- [ ] 3.1.5 Implement index history query (from `index_runs` table - Phase 1)
- [ ] 3.1.6 Implement Claude CLI detection
- [ ] 3.1.7 Add unit tests for all calculations
- [ ] 3.1.8 Add integration test for endpoint

---

### 3.2 Storage/Performance/Health Cards

**Description:** Three summary cards at the top of the System page showing key metrics.

**File Changes:**

| File | Change |
|------|--------|
| `src/components/SystemPage.tsx` | New page component |
| `src/components/SystemStorageCard.tsx` | Storage metrics card |
| `src/components/SystemPerformanceCard.tsx` | Performance metrics card |
| `src/components/SystemHealthCard.tsx` | Health status card |
| `src/hooks/use-system.ts` | React Query hook for `/api/system` |
| `src/router.tsx` | Add `/system` route |
| `src/components/Sidebar.tsx` | Add "System" nav item |

**Subtasks:**

- [ ] 3.2.1 Create `use-system` hook with React Query
- [ ] 3.2.2 Create `SystemStorageCard` with size formatting
- [ ] 3.2.3 Create `SystemPerformanceCard` with throughput display
- [ ] 3.2.4 Create `SystemHealthCard` with status indicator
- [ ] 3.2.5 Create main `SystemPage` layout
- [ ] 3.2.6 Add route and navigation
- [ ] 3.2.7 Add loading/error states

---

### 3.3 Classification Status Section

**Description:** Section showing classification progress and provider configuration.

**File Changes:**

| File | Change |
|------|--------|
| `src/components/SystemClassificationSection.tsx` | New component |
| `src/components/ClassificationConfigModal.tsx` | Provider configuration modal |

**Subtasks:**

- [ ] 3.3.1 Create classification status display (classified/total counts)
- [ ] 3.3.2 Show last run timestamp and duration
- [ ] 3.3.3 Add estimated cost display (based on haiku pricing)
- [ ] 3.3.4 Add "Classify Remaining" button (calls Phase 2 endpoint)
- [ ] 3.3.5 Create provider configuration modal
- [ ] 3.3.6 Handle classification-in-progress state (disable button, show progress)

---

### 3.4 Index History Table

**Description:** Table showing recent index runs with details.

**File Changes:**

| File | Change |
|------|--------|
| `src/components/SystemIndexHistory.tsx` | New component |

**Subtasks:**

- [ ] 3.4.1 Create table component with columns
- [ ] 3.4.2 Add row status indicator (success/failed)
- [ ] 3.4.3 Add pagination or "show more" for long history
- [ ] 3.4.4 Format timestamps as relative time
- [ ] 3.4.5 Format duration in human-readable format

---

### 3.5 Action Buttons

**Description:** Row of action buttons for system maintenance operations.

**File Changes:**

| File | Change |
|------|--------|
| `src/components/SystemActions.tsx` | New component with action buttons |
| `crates/server/src/routes/system.rs` | Add POST endpoints for actions |
| `crates/db/src/queries.rs` | Add clear cache, reset functions |

**Subtasks:**

- [ ] 3.5.1 Create action button row component
- [ ] 3.5.2 Implement "Re-index All" (triggers full index rebuild)
- [ ] 3.5.3 Implement "Clear Cache" (resets index & search)
- [ ] 3.5.4 Implement "Export Data" (JSON/CSV download)
- [ ] 3.5.5 Implement "Git Re-sync" (refresh commit correlations)
- [ ] 3.5.6 Implement "Reset All" (factory reset with confirmation)
- [ ] 3.5.7 Add confirmation modal for destructive actions
- [ ] 3.5.8 Show loading state during operations
- [ ] 3.5.9 Show success/error toast notifications

---

### 3.6 Claude CLI Status Check

**Description:** Detect Claude CLI installation, version, and authentication status.

**File Changes:**

| File | Change |
|------|--------|
| `crates/core/src/cli.rs` | New module for CLI detection |
| `crates/core/src/lib.rs` | Export CLI module |
| `src/components/SystemCliStatus.tsx` | CLI status display component |

**Subtasks:**

- [ ] 3.6.1 Implement `which claude` detection
- [ ] 3.6.2 Parse `claude --version` output
- [ ] 3.6.3 Check `claude auth status` (or equivalent)
- [ ] 3.6.4 Parse subscription type from auth output
- [ ] 3.6.5 Create CLI status display component
- [ ] 3.6.6 Show installation instructions if CLI not found
- [ ] 3.6.7 Handle timeout for CLI commands

---

## API Specification

### GET /api/system

Returns comprehensive system status information.

**Request:**
```
GET /api/system
```

**Response:**
```typescript
interface SystemResponse {
  storage: {
    // Total size of all JSONL session files
    jsonlBytes: number;
    // Size of Tantivy search index (if applicable)
    indexBytes: number;
    // Size of SQLite database file
    dbBytes: number;
    // Size of any cached data
    cacheBytes: number;
    // Sum of all above
    totalBytes: number;
  };
  performance: {
    // Duration of last successful index in milliseconds
    lastIndexDuration: number;
    // Throughput: bytes processed per second during last index
    throughputBytesPerSec: number;
    // Sessions indexed per second during last index
    sessionsPerSec: number;
  };
  health: {
    // Total number of sessions in database
    sessionsCount: number;
    // Total number of commits tracked
    commitsCount: number;
    // Total number of unique projects
    projectsCount: number;
    // Number of parsing/indexing errors in last run
    errorsCount: number;
    // Timestamp of last successful sync
    lastSyncAt: string; // ISO 8601
    // Overall system status
    status: 'healthy' | 'warning' | 'error';
  };
  indexHistory: Array<{
    // When the index run started
    timestamp: string; // ISO 8601
    // Type of index operation
    type: 'full' | 'incremental';
    // Number of sessions indexed
    sessionsCount: number;
    // Duration in milliseconds
    duration: number;
    // Result status
    status: 'success' | 'failed';
    // Error message if failed
    errorMessage?: string;
  }>;
  classification: {
    // Number of sessions with classification
    classifiedCount: number;
    // Number of sessions without classification
    unclassifiedCount: number;
    // Timestamp of last classification run
    lastRunAt: string | null; // ISO 8601
    // Duration of last run in milliseconds
    lastRunDuration: number | null;
    // Estimated cost of last run (USD)
    lastRunCost: number | null;
    // Current provider configuration
    provider: 'claude-cli' | 'anthropic-api' | 'openai-compatible';
    // Model used for classification
    model: string;
    // Whether a classification job is currently running
    isRunning: boolean;
    // Progress if running (0-100)
    progress: number | null;
  };
  claudeCli: {
    // Path to claude binary, null if not found
    path: string | null;
    // Version string, null if not found
    version: string | null;
    // Whether CLI is authenticated
    authenticated: boolean;
    // Subscription type if authenticated
    subscriptionType: string | null; // 'free' | 'pro' | 'team' | 'enterprise'
  };
}
```

**Response Example:**
```json
{
  "storage": {
    "jsonlBytes": 13194139648,
    "indexBytes": 888020992,
    "dbBytes": 163577856,
    "cacheBytes": 0,
    "totalBytes": 14245738496
  },
  "performance": {
    "lastIndexDuration": 2800,
    "throughputBytesPerSec": 4212457088,
    "sessionsPerSec": 2397
  },
  "health": {
    "sessionsCount": 6712,
    "commitsCount": 1234,
    "projectsCount": 47,
    "errorsCount": 0,
    "lastSyncAt": "2026-02-05T14:34:00Z",
    "status": "healthy"
  },
  "indexHistory": [
    {
      "timestamp": "2026-02-05T14:34:00Z",
      "type": "full",
      "sessionsCount": 6712,
      "duration": 2800,
      "status": "success"
    },
    {
      "timestamp": "2026-02-05T10:12:00Z",
      "type": "incremental",
      "sessionsCount": 47,
      "duration": 300,
      "status": "success"
    }
  ],
  "classification": {
    "classifiedCount": 847,
    "unclassifiedCount": 5865,
    "lastRunAt": "2026-02-04T14:34:00Z",
    "lastRunDuration": 503000,
    "lastRunCost": 1.82,
    "provider": "claude-cli",
    "model": "claude-3-haiku-20240307",
    "isRunning": false,
    "progress": null
  },
  "claudeCli": {
    "path": "/opt/homebrew/bin/claude",
    "version": "1.0.12",
    "authenticated": true,
    "subscriptionType": "pro"
  }
}
```

### POST /api/system/reindex

Triggers a full re-index of all session files.

**Request:**
```
POST /api/system/reindex
```

**Response:**
```json
{
  "status": "started",
  "message": "Full re-index started"
}
```

### POST /api/system/clear-cache

Clears the search index and cached data.

**Request:**
```
POST /api/system/clear-cache
```

**Response:**
```json
{
  "status": "success",
  "clearedBytes": 888020992
}
```

### POST /api/system/git-resync

Triggers a full refresh of git commit correlations.

**Request:**
```
POST /api/system/git-resync
```

**Response:**
```json
{
  "status": "started",
  "message": "Git re-sync started"
}
```

### POST /api/system/reset

Factory reset - clears all data. Requires confirmation token.

**Request:**
```json
{
  "confirm": "RESET_ALL_DATA"
}
```

**Response:**
```json
{
  "status": "success",
  "message": "All data has been reset"
}
```

---

## UI Mockups

### Full Page Layout

```
+-----------------------------------------------------------------------------+
|  System Status                                                              |
+-----------------------------------------------------------------------------+
|                                                                             |
|  +-------------------+  +-------------------+  +-------------------+        |
|  |  STORAGE          |  |  PERFORMANCE      |  |  DATA HEALTH      |        |
|  |  JSONL    12.3 GB |  |  Last index  2.8s |  |  Sessions   6,712 |        |
|  |  Index     847 MB |  |  4.2 GB/s         |  |  Commits    1,234 |        |
|  |  DB        156 MB |  |  2,397 sess/sec   |  |  Projects      47 |        |
|  |  Cache       0 MB |  |                   |  |  Errors         0 |        |
|  |  --------------- |  |                   |  |  --------------- |        |
|  |  Total   13.3 GB |  |                   |  |  Status: Healthy  |        |
|  +-------------------+  +-------------------+  +-------------------+        |
|                                                                             |
|  AI Classification                                                          |
|  +-----------------------------------------------------------------------+  |
|  |  Classified: 847 / 6,712 (12.6%)  [=======>                        ]  |  |
|  |                                                                       |  |
|  |  Provider: Claude CLI (haiku)      Last run: Feb 4, 2:34 PM          |  |
|  |  Duration: 8m 23s                  Est. cost: ~$1.82                  |  |
|  |                                                                       |  |
|  |  [ Classify Remaining ]  [ Configure Provider ]                       |  |
|  +-----------------------------------------------------------------------+  |
|                                                                             |
|  Index History                                                              |
|  +-----------------------------------------------------------------------+  |
|  |  Time              Type          Sessions    Duration    Status       |  |
|  |  -------------------------------------------------------------------- |  |
|  |  Today 2:34 PM     Full          6,712       2.8s        [x] Success  |  |
|  |  Today 10:12 AM    Incremental   +47         0.3s        [x] Success  |  |
|  |  Yesterday 6:00 PM Full          6,665       2.7s        [x] Success  |  |
|  |  Yesterday 9:15 AM Incremental   +23         0.2s        [!] Failed   |  |
|  |                                                                       |  |
|  |                              [ Show More ]                            |  |
|  +-----------------------------------------------------------------------+  |
|                                                                             |
|  Actions                                                                    |
|  +-----------------------------------------------------------------------+  |
|  |  [ Re-index All ]  [ Clear Cache ]  [ Export Data ]  [ Git Re-sync ] |  |
|  |                                                                       |  |
|  |  [ Reset All... ]  <-- Destructive, requires confirmation             |  |
|  +-----------------------------------------------------------------------+  |
|                                                                             |
|  Claude CLI                                                                 |
|  +-----------------------------------------------------------------------+  |
|  |  [x] Installed: /opt/homebrew/bin/claude                              |  |
|  |  Version: 1.0.12                                                      |  |
|  |  [x] Authenticated (Pro subscription)                                 |  |
|  +-----------------------------------------------------------------------+  |
|                                                                             |
+-----------------------------------------------------------------------------+
```

### Storage Card States

```
Normal State:
+-------------------+
|  STORAGE          |
|  JSONL    12.3 GB |
|  Index     847 MB |
|  DB        156 MB |
|  Cache       0 MB |
|  --------------- |
|  Total   13.3 GB |
+-------------------+

Low Disk Warning (< 1GB free):
+-------------------+
|  STORAGE     [!]  |
|  JSONL    12.3 GB |
|  Index     847 MB |
|  DB        156 MB |
|  Cache       0 MB |
|  --------------- |
|  Total   13.3 GB |
|                   |
|  Low disk space!  |
+-------------------+
```

### Health Card States

```
Healthy:
+-------------------+
|  DATA HEALTH      |
|  Sessions   6,712 |
|  Commits    1,234 |
|  Projects      47 |
|  Errors         0 |
|  --------------- |
|  [x] Healthy      |
+-------------------+

Warning (errors > 0, < 10):
+-------------------+
|  DATA HEALTH [!]  |
|  Sessions   6,710 |
|  Commits    1,234 |
|  Projects      47 |
|  Errors         2 |
|  --------------- |
|  [!] 2 parse errs |
+-------------------+

Error (errors >= 10 or index stale > 24h):
+-------------------+
|  DATA HEALTH [X]  |
|  Sessions   6,500 |
|  Commits    1,100 |
|  Projects      45 |
|  Errors        15 |
|  --------------- |
|  [X] Index stale  |
+-------------------+
```

### Classification Section States

```
Idle (has unclassified):
+-----------------------------------------------------------------------+
|  AI Classification                                                     |
|                                                                        |
|  Classified: 847 / 6,712 (12.6%)  [=======>                        ]  |
|                                                                        |
|  Provider: Claude CLI (haiku)      Last run: Feb 4, 2:34 PM           |
|  Duration: 8m 23s                  Est. cost: ~$1.82                   |
|                                                                        |
|  [ Classify Remaining ]  [ Configure Provider ]                        |
+-----------------------------------------------------------------------+

Running:
+-----------------------------------------------------------------------+
|  AI Classification                                                     |
|                                                                        |
|  Classifying: 1,247 / 5,865 (21.3%)  [=========>                   ]  |
|                                                                        |
|  Provider: Claude CLI (haiku)      Started: 2 min ago                 |
|  Elapsed: 2m 15s                   Est. remaining: ~8m                 |
|                                                                        |
|  [ Cancel ]  [ Configure Provider ]                                    |
+-----------------------------------------------------------------------+

Complete (all classified):
+-----------------------------------------------------------------------+
|  AI Classification                                                     |
|                                                                        |
|  Classified: 6,712 / 6,712 (100%)  [================================] |
|                                                                        |
|  Provider: Claude CLI (haiku)      Last run: Feb 5, 10:12 AM          |
|  Duration: 45m 12s                 Total cost: ~$12.50                 |
|                                                                        |
|  [x] All sessions classified  [ Configure Provider ]                   |
+-----------------------------------------------------------------------+
```

### CLI Status States

```
Installed and Authenticated:
+-----------------------------------------------------------------------+
|  Claude CLI                                                            |
|                                                                        |
|  [x] Installed: /opt/homebrew/bin/claude                               |
|  Version: 1.0.12                                                       |
|  [x] Authenticated (Pro subscription)                                  |
+-----------------------------------------------------------------------+

Installed but Not Authenticated:
+-----------------------------------------------------------------------+
|  Claude CLI                                                            |
|                                                                        |
|  [x] Installed: /opt/homebrew/bin/claude                               |
|  Version: 1.0.12                                                       |
|  [!] Not authenticated                                                 |
|                                                                        |
|  Run: claude auth login                                                |
+-----------------------------------------------------------------------+

Not Installed:
+-----------------------------------------------------------------------+
|  Claude CLI                                                            |
|                                                                        |
|  [X] Not installed                                                     |
|                                                                        |
|  Install Claude CLI to enable AI classification:                       |
|  npm install -g @anthropic-ai/claude-cli                               |
|  # or                                                                  |
|  brew install claude                                                   |
+-----------------------------------------------------------------------+
```

### Reset Confirmation Modal

```
+---------------------------------------------------------------+
|  Reset All Data                                               |
+---------------------------------------------------------------+
|                                                               |
|  [!] This action cannot be undone.                            |
|                                                               |
|  This will permanently delete:                                |
|  - All session metadata and indexes                           |
|  - All commit correlations                                    |
|  - All classification data                                    |
|  - All cached data                                            |
|                                                               |
|  Your original JSONL files will NOT be deleted.               |
|                                                               |
|  Type "RESET_ALL_DATA" to confirm:                            |
|  +-------------------------------------------------------+    |
|  |                                                       |    |
|  +-------------------------------------------------------+    |
|                                                               |
|                        [ Cancel ]  [ Reset All ]              |
+---------------------------------------------------------------+
```

---

## React Components

### Component Hierarchy

```
SystemPage
├── SystemHeader
│   └── PageTitle
├── SystemMetricsGrid (3-column)
│   ├── SystemStorageCard
│   │   ├── MetricRow (JSONL)
│   │   ├── MetricRow (Index)
│   │   ├── MetricRow (DB)
│   │   ├── MetricRow (Cache)
│   │   ├── Divider
│   │   └── MetricRow (Total)
│   ├── SystemPerformanceCard
│   │   ├── MetricRow (Last index)
│   │   ├── MetricRow (Throughput)
│   │   └── MetricRow (Sessions/sec)
│   └── SystemHealthCard
│       ├── MetricRow (Sessions)
│       ├── MetricRow (Commits)
│       ├── MetricRow (Projects)
│       ├── MetricRow (Errors)
│       ├── Divider
│       └── StatusIndicator
├── SystemClassificationSection
│   ├── ProgressBar
│   ├── MetricGrid
│   │   ├── Provider/Model
│   │   ├── Last Run
│   │   ├── Duration
│   │   └── Cost
│   └── ActionButtons
│       ├── ClassifyButton
│       └── ConfigureButton
├── SystemIndexHistory
│   ├── Table
│   │   └── TableRow[] (timestamp, type, count, duration, status)
│   └── ShowMoreButton
├── SystemActions
│   ├── ActionButton (Re-index)
│   ├── ActionButton (Clear Cache)
│   ├── ActionButton (Export)
│   ├── ActionButton (Git Re-sync)
│   └── ActionButton (Reset - danger)
├── SystemCliStatus
│   ├── StatusIndicator (installed)
│   ├── PathDisplay
│   ├── VersionDisplay
│   ├── AuthStatus
│   └── InstallInstructions (conditional)
└── ClassificationConfigModal (portal)
    ├── ProviderSelect
    ├── ModelSelect
    ├── ApiKeyInput (conditional)
    └── ActionButtons
```

### Component Props

```typescript
// src/components/SystemPage.tsx
interface SystemPageProps {
  // No props - uses useSystem() hook internally
}

// src/components/SystemStorageCard.tsx
interface SystemStorageCardProps {
  storage: {
    jsonlBytes: number;
    indexBytes: number;
    dbBytes: number;
    cacheBytes: number;
    totalBytes: number;
  };
  isLoading?: boolean;
}

// src/components/SystemPerformanceCard.tsx
interface SystemPerformanceCardProps {
  performance: {
    lastIndexDuration: number;
    throughputBytesPerSec: number;
    sessionsPerSec: number;
  };
  isLoading?: boolean;
}

// src/components/SystemHealthCard.tsx
interface SystemHealthCardProps {
  health: {
    sessionsCount: number;
    commitsCount: number;
    projectsCount: number;
    errorsCount: number;
    lastSyncAt: string;
    status: 'healthy' | 'warning' | 'error';
  };
  isLoading?: boolean;
}

// src/components/SystemClassificationSection.tsx
interface SystemClassificationSectionProps {
  classification: {
    classifiedCount: number;
    unclassifiedCount: number;
    lastRunAt: string | null;
    lastRunDuration: number | null;
    lastRunCost: number | null;
    provider: string;
    model: string;
    isRunning: boolean;
    progress: number | null;
  };
  onClassify: () => void;
  onConfigure: () => void;
  onCancel: () => void;
}

// src/components/SystemIndexHistory.tsx
interface SystemIndexHistoryProps {
  history: Array<{
    timestamp: string;
    type: 'full' | 'incremental';
    sessionsCount: number;
    duration: number;
    status: 'success' | 'failed';
    errorMessage?: string;
  }>;
  isLoading?: boolean;
}

// src/components/SystemActions.tsx
interface SystemActionsProps {
  onReindex: () => void;
  onClearCache: () => void;
  onExport: () => void;
  onGitResync: () => void;
  onReset: () => void;
  isReindexing?: boolean;
  isClearingCache?: boolean;
  isExporting?: boolean;
  isResyncing?: boolean;
}

// src/components/SystemCliStatus.tsx
interface SystemCliStatusProps {
  cli: {
    path: string | null;
    version: string | null;
    authenticated: boolean;
    subscriptionType: string | null;
  };
  isLoading?: boolean;
}

// src/components/ClassificationConfigModal.tsx
interface ClassificationConfigModalProps {
  isOpen: boolean;
  onClose: () => void;
  currentProvider: string;
  currentModel: string;
  onSave: (config: ClassificationConfig) => void;
}

interface ClassificationConfig {
  provider: 'claude-cli' | 'anthropic-api' | 'openai-compatible';
  model: string;
  apiKey?: string;
  baseUrl?: string;
}
```

---

## Rust Implementation

### Storage Calculation

```rust
// crates/db/src/queries.rs

/// Storage statistics for the system page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub jsonl_bytes: u64,
    pub index_bytes: u64,
    pub db_bytes: u64,
    pub cache_bytes: u64,
    pub total_bytes: u64,
}

impl Database {
    /// Calculate storage statistics.
    ///
    /// - jsonl_bytes: Sum of all session file sizes from indexer_state
    /// - db_bytes: Size of the SQLite database file
    /// - index_bytes: Size of Tantivy index directory (if exists)
    /// - cache_bytes: Size of any other cache files
    pub async fn get_storage_stats(&self) -> DbResult<StorageStats> {
        // Sum of JSONL file sizes from indexer_state
        let (jsonl_bytes,): (i64,) = sqlx::query_as(
            "SELECT COALESCE(SUM(file_size), 0) FROM indexer_state"
        )
        .fetch_one(self.pool())
        .await?;

        // Database file size
        let db_bytes = if self.db_path().exists() {
            std::fs::metadata(self.db_path())
                .map(|m| m.len())
                .unwrap_or(0)
        } else {
            0
        };

        // Index directory size (Tantivy)
        let index_bytes = self.calculate_index_size().await;

        // Cache directory size
        let cache_bytes = self.calculate_cache_size().await;

        let total_bytes = jsonl_bytes as u64 + index_bytes + db_bytes + cache_bytes;

        Ok(StorageStats {
            jsonl_bytes: jsonl_bytes as u64,
            index_bytes,
            db_bytes,
            cache_bytes,
            total_bytes,
        })
    }

    /// Calculate the size of the Tantivy index directory.
    async fn calculate_index_size(&self) -> u64 {
        let cache_dir = match dirs::cache_dir() {
            Some(d) => d.join("claude-view").join("index"),
            None => return 0,
        };

        if !cache_dir.exists() {
            return 0;
        }

        walkdir::WalkDir::new(&cache_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
            .sum()
    }

    /// Calculate the size of cache files.
    async fn calculate_cache_size(&self) -> u64 {
        // Currently no separate cache, but structure for future use
        0
    }
}
```

### Health Statistics

```rust
// crates/db/src/queries.rs

/// Health statistics for the system page.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct HealthStats {
    pub sessions_count: i64,
    pub commits_count: i64,
    pub projects_count: i64,
    pub errors_count: i64,
    pub last_sync_at: Option<i64>,
    pub status: HealthStatus,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Warning,
    Error,
}

impl Database {
    /// Get health statistics for the system page.
    pub async fn get_health_stats(&self) -> DbResult<HealthStats> {
        // Count sessions (excluding sidechains)
        let (sessions_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE is_sidechain = 0"
        )
        .fetch_one(self.pool())
        .await?;

        // Count unique commits
        let (commits_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM commits"
        )
        .fetch_one(self.pool())
        .await?;

        // Count unique projects
        let (projects_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(DISTINCT project_id) FROM sessions"
        )
        .fetch_one(self.pool())
        .await?;

        // Count parsing errors (sessions with parse_version = 0 that should have been indexed)
        let (errors_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sessions WHERE parse_version = 0 AND deep_indexed_at IS NOT NULL"
        )
        .fetch_one(self.pool())
        .await?;

        // Get last sync timestamp
        let metadata = self.get_index_metadata().await?;
        let last_sync_at = metadata.last_indexed_at;

        // Determine status
        let status = self.calculate_health_status(errors_count, last_sync_at).await;

        Ok(HealthStats {
            sessions_count,
            commits_count,
            projects_count,
            errors_count,
            last_sync_at,
            status,
        })
    }

    /// Calculate health status based on errors and staleness.
    async fn calculate_health_status(
        &self,
        errors_count: i64,
        last_sync_at: Option<i64>,
    ) -> HealthStatus {
        // Error: 10+ errors or index stale > 24 hours
        if errors_count >= 10 {
            return HealthStatus::Error;
        }

        if let Some(ts) = last_sync_at {
            let now = chrono::Utc::now().timestamp();
            let hours_stale = (now - ts) / 3600;
            if hours_stale >= 24 {
                return HealthStatus::Error;
            }
        }

        // Warning: any errors
        if errors_count > 0 {
            return HealthStatus::Warning;
        }

        HealthStatus::Healthy
    }
}
```

### Claude CLI Detection

```rust
// crates/core/src/cli.rs

use std::process::Command;
use serde::Serialize;
use ts_rs::TS;

/// Claude CLI status information.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCliStatus {
    /// Path to claude binary, None if not found.
    pub path: Option<String>,
    /// Version string, None if not found.
    pub version: Option<String>,
    /// Whether CLI is authenticated.
    pub authenticated: bool,
    /// Subscription type if authenticated.
    pub subscription_type: Option<String>,
}

impl ClaudeCliStatus {
    /// Detect Claude CLI installation and status.
    ///
    /// Runs with a 5-second timeout for each command.
    pub fn detect() -> Self {
        let path = Self::find_claude_path();

        if path.is_none() {
            return Self {
                path: None,
                version: None,
                authenticated: false,
                subscription_type: None,
            };
        }

        let version = Self::get_version(&path.as_ref().unwrap());
        let (authenticated, subscription_type) = Self::check_auth(&path.as_ref().unwrap());

        Self {
            path,
            version,
            authenticated,
            subscription_type,
        }
    }

    /// Find the path to the claude binary.
    fn find_claude_path() -> Option<String> {
        let output = Command::new("which")
            .arg("claude")
            .output()
            .ok()?;

        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }

        // Fallback: check common locations
        let common_paths = [
            "/opt/homebrew/bin/claude",
            "/usr/local/bin/claude",
            "/usr/bin/claude",
        ];

        for path in common_paths {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    /// Get the claude CLI version.
    fn get_version(path: &str) -> Option<String> {
        let output = Command::new(path)
            .arg("--version")
            .output()
            .ok()?;

        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Parse version from output like "claude version 1.0.12"
            if let Some(v) = version_str.split_whitespace().last() {
                return Some(v.to_string());
            }
        }

        None
    }

    /// Check authentication status.
    fn check_auth(path: &str) -> (bool, Option<String>) {
        // Try to get auth status
        // This may vary based on actual claude CLI implementation
        let output = Command::new(path)
            .args(["auth", "status"])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                let output_str = String::from_utf8_lossy(&o.stdout);
                // Parse subscription type from output
                // Example: "Authenticated as user@email.com (Pro)"
                let subscription = Self::parse_subscription_type(&output_str);
                (true, subscription)
            }
            _ => (false, None),
        }
    }

    /// Parse subscription type from auth status output.
    fn parse_subscription_type(output: &str) -> Option<String> {
        // Look for patterns like "(Pro)", "(Free)", "(Team)", "(Enterprise)"
        let types = ["pro", "free", "team", "enterprise"];
        let lower = output.to_lowercase();

        for t in types {
            if lower.contains(&format!("({})", t)) {
                return Some(t.to_string());
            }
        }

        // Fallback: check if authenticated at all
        if lower.contains("authenticated") {
            return Some("unknown".to_string());
        }

        None
    }
}
```

### System Endpoint

```rust
// crates/server/src/routes/system.rs

use std::sync::Arc;
use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use claude_view_core::ClaudeCliStatus;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Full system status response.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export, export_to = "../../../src/types/generated/")]
#[serde(rename_all = "camelCase")]
pub struct SystemResponse {
    pub storage: StorageInfo,
    pub performance: PerformanceInfo,
    pub health: HealthInfo,
    pub index_history: Vec<IndexRunInfo>,
    pub classification: ClassificationInfo,
    pub claude_cli: ClaudeCliStatus,
}

// ... (nested types as shown in API spec)

/// GET /api/system - Get comprehensive system status.
pub async fn get_system_status(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<SystemResponse>> {
    // Get storage stats
    let storage_stats = state.db.get_storage_stats().await?;

    // Get health stats
    let health_stats = state.db.get_health_stats().await?;

    // Get index metadata for performance metrics
    let metadata = state.db.get_index_metadata().await?;

    // Get index history (requires Phase 1 table)
    let index_history = state.db.get_index_history(10).await?;

    // Get classification status (requires Phase 1 table)
    let classification = state.db.get_classification_status().await?;

    // Detect Claude CLI (runs shell commands - fast, cached)
    let claude_cli = ClaudeCliStatus::detect();

    // Calculate performance metrics
    let performance = calculate_performance(&metadata, &storage_stats);

    // Convert to response types
    let response = SystemResponse {
        storage: storage_stats.into(),
        performance,
        health: health_stats.into(),
        index_history: index_history.into_iter().map(Into::into).collect(),
        classification: classification.into(),
        claude_cli,
    };

    Ok(Json(response))
}

/// POST /api/system/reindex - Trigger full re-index.
pub async fn trigger_reindex(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    // Trigger background reindex via state.indexing_state
    state.trigger_full_reindex().await?;

    Ok(Json(ActionResponse {
        status: "started".to_string(),
        message: Some("Full re-index started".to_string()),
    }))
}

/// POST /api/system/clear-cache - Clear search index and cache.
pub async fn clear_cache(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ClearCacheResponse>> {
    let cleared_bytes = state.clear_cache().await?;

    Ok(Json(ClearCacheResponse {
        status: "success".to_string(),
        cleared_bytes,
    }))
}

/// POST /api/system/git-resync - Trigger full git re-sync.
pub async fn trigger_git_resync(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<ActionResponse>> {
    state.trigger_git_resync().await?;

    Ok(Json(ActionResponse {
        status: "started".to_string(),
        message: Some("Git re-sync started".to_string()),
    }))
}

/// POST /api/system/reset - Factory reset all data.
#[derive(Debug, Deserialize)]
pub struct ResetRequest {
    confirm: String,
}

pub async fn reset_all(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResetRequest>,
) -> ApiResult<Json<ActionResponse>> {
    // Require exact confirmation string
    if body.confirm != "RESET_ALL_DATA" {
        return Err(ApiError::BadRequest(
            "Invalid confirmation. Type 'RESET_ALL_DATA' to confirm.".to_string()
        ));
    }

    state.reset_all_data().await?;

    Ok(Json(ActionResponse {
        status: "success".to_string(),
        message: Some("All data has been reset".to_string()),
    }))
}

/// Create the system routes router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/system", get(get_system_status))
        .route("/system/reindex", post(trigger_reindex))
        .route("/system/clear-cache", post(clear_cache))
        .route("/system/git-resync", post(trigger_git_resync))
        .route("/system/reset", post(reset_all))
}
```

---

## Testing Strategy

### Unit Tests

| Test | Location | Description |
|------|----------|-------------|
| `test_storage_stats_empty_db` | `crates/db/src/queries.rs` | Storage stats with no data |
| `test_storage_stats_with_data` | `crates/db/src/queries.rs` | Storage stats with sample data |
| `test_health_stats_healthy` | `crates/db/src/queries.rs` | Health status = healthy |
| `test_health_stats_warning` | `crates/db/src/queries.rs` | Health status = warning (1-9 errors) |
| `test_health_stats_error` | `crates/db/src/queries.rs` | Health status = error (10+ errors) |
| `test_health_stats_stale` | `crates/db/src/queries.rs` | Health status = error (>24h stale) |
| `test_cli_detect_not_installed` | `crates/core/src/cli.rs` | CLI not found |
| `test_cli_detect_version_parse` | `crates/core/src/cli.rs` | Version parsing |
| `test_cli_subscription_parse` | `crates/core/src/cli.rs` | Subscription type parsing |

### Integration Tests

| Test | Location | Description |
|------|----------|-------------|
| `test_system_endpoint_empty_db` | `crates/server/src/routes/system.rs` | Full response with empty DB |
| `test_system_endpoint_with_data` | `crates/server/src/routes/system.rs` | Full response with sample data |
| `test_reindex_endpoint` | `crates/server/src/routes/system.rs` | Trigger reindex |
| `test_clear_cache_endpoint` | `crates/server/src/routes/system.rs` | Clear cache |
| `test_reset_requires_confirmation` | `crates/server/src/routes/system.rs` | Reset without confirmation fails |
| `test_reset_with_confirmation` | `crates/server/src/routes/system.rs` | Reset with confirmation succeeds |

### React Component Tests

| Test | Location | Description |
|------|----------|-------------|
| `renders storage card` | `SystemStorageCard.test.tsx` | Renders with data |
| `formats bytes correctly` | `SystemStorageCard.test.tsx` | 12.3 GB, 847 MB, etc. |
| `renders performance card` | `SystemPerformanceCard.test.tsx` | Renders with data |
| `formats throughput` | `SystemPerformanceCard.test.tsx` | 4.2 GB/s, etc. |
| `renders health card healthy` | `SystemHealthCard.test.tsx` | Green status |
| `renders health card warning` | `SystemHealthCard.test.tsx` | Yellow status |
| `renders health card error` | `SystemHealthCard.test.tsx` | Red status |
| `classification idle state` | `SystemClassificationSection.test.tsx` | Shows button |
| `classification running state` | `SystemClassificationSection.test.tsx` | Shows progress |
| `classification complete state` | `SystemClassificationSection.test.tsx` | Shows checkmark |
| `index history table` | `SystemIndexHistory.test.tsx` | Renders rows |
| `action buttons trigger handlers` | `SystemActions.test.tsx` | Click handlers called |
| `reset shows confirmation` | `SystemActions.test.tsx` | Modal appears |
| `cli installed state` | `SystemCliStatus.test.tsx` | Shows path/version |
| `cli not installed state` | `SystemCliStatus.test.tsx` | Shows instructions |

### E2E Tests (Playwright)

| Test | Description |
|------|-------------|
| `system page loads` | Navigate to /system, verify cards render |
| `reindex button works` | Click reindex, verify toast |
| `export data downloads` | Click export, verify file download |
| `reset flow` | Click reset, type confirmation, verify success |

---

## Acceptance Criteria

### Must Have

- [ ] `/api/system` endpoint returns all specified data
- [ ] Storage card shows JSONL, Index, DB, Cache, Total sizes
- [ ] Performance card shows last index duration, throughput, sessions/sec
- [ ] Health card shows counts and status indicator
- [ ] Index history table shows recent runs
- [ ] All action buttons trigger their respective operations
- [ ] Reset requires "RESET_ALL_DATA" confirmation
- [ ] Claude CLI status displays correctly for installed/not-installed states
- [ ] Page loads in < 500ms
- [ ] All tests pass

### Should Have

- [ ] Classification section shows progress and provider info
- [ ] Provider configuration modal works
- [ ] Toast notifications for action success/failure
- [ ] Loading states for all cards
- [ ] Error states for failed API calls

### Nice to Have

- [ ] Real-time SSE updates during classification
- [ ] Disk space warning when < 1GB free
- [ ] Export format selection (JSON/CSV)
- [ ] Index history pagination

---

## Performance Considerations

1. **CLI Detection Caching**: Cache `ClaudeCliStatus::detect()` result for 30 seconds to avoid repeated shell spawns.

2. **Storage Calculation**: Use filesystem stats, not DB queries for file sizes. The `indexer_state.file_size` sum is fast.

3. **Directory Size Calculation**: Use `walkdir` crate for efficient recursive size calculation. Consider caching for large index directories.

4. **Parallel Queries**: Run storage, health, and classification queries in parallel using `tokio::join!`.

5. **Index History Limit**: Default to 10 recent runs, paginate if more needed.

---

## Related Documents

- Master Design: `../2026-02-05-theme4-chat-insights-design.md`
- Phase 1 (dependency): `phase1-foundation.md`
- Phase 2 (parallel): `phase2-classification.md`
- Existing Settings Page: `src/components/SettingsPage.tsx`
- Existing Status Endpoint: `crates/server/src/routes/status.rs`
