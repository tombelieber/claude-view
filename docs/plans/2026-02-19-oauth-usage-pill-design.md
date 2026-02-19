# OAuth Usage Pill - Design Document

## Overview

Add a compact OAuth usage indicator to the Mission Control page header, displaying Claude Code subscription consumption near the "Live" connection status.

## Requirements

### Display Elements
1. **Progress dots** - 10 segments, filled proportionally to usage percentage
2. **Percentage** - e.g., `62%`
3. **Budget remaining** - e.g., `$50/$80`
4. **Days until reset** - e.g., `12d`

### States
| State | Display |
|-------|---------|
| Loading | `Loading usage...` |
| Error | `Usage unavailable` (tooltip with error) |
| No auth | Hidden entirely (no OAuth credentials) |
| Success | Full pill with all metrics |

### Behavior
- Poll on mount + interval (reuse `autoUpdateInterval` from App.tsx: 5/15/30/60 min)
- Tooltip on hover showing last fetch time
- Fetch via existing Tauri plugin: `plugins/claude/plugin.js`

## Placement

**Mission Control page header** — inline with connection status:

```
Mission Control  [grid|list|kanban|monitor]          ● Live · ●●●●●○○○○○ 62% · $50/$80 · 12d
```

Location: `src/pages/MissionControlPage.tsx:156-166` (right side of header)

## Architecture

### Component Structure
```
src/components/live/OAuthUsagePill.tsx
├── OAuthUsagePill (main component)
├── ProgressDots (10-segment visual)
└── useOAuthUsage (hook for fetching/polling)
```

### Data Flow
1. `useOAuthUsage` hook calls Tauri command (via existing plugin)
2. Plugin reads `~/.claude/.credentials.json`
3. Calls `https://api.anthropic.com/api/oauth/usage`
4. Returns `{ usedCents, limitCents, resetDate }`
5. Component renders pill or hides if no auth

### Integration Points
- **Hook location**: `src/hooks/use-oauth-usage.ts`
- **Component**: `src/components/live/OAuthUsagePill.tsx`
- **Usage**: Import and render in `MissionControlPage.tsx:156-166`

## API Contract

### Tauri Command (existing)
```rust
// From plugins/claude/plugin.js - already implements this
#[tauri::command]
async fn fetch_oauth_usage() -> Result<OAuthUsage, Error>
```

### Response Shape
```typescript
interface OAuthUsage {
  usedCents: number      // e.g., 5000 = $50.00
  limitCents: number     // e.g., 8000 = $80.00
  resetDate: string      // ISO date string
  error?: string         // If fetch failed
  hasAuth: boolean       // False if no credentials
}
```

## Error Handling
- Network errors → show "Usage unavailable" with retry tooltip
- Auth errors → hide pill entirely
- Partial data → show what's available, hide missing fields

## Testing
- Unit test: `OAuthUsagePill.test.tsx`
  - Renders loading state
  - Renders success state with all metrics
  - Hides when no auth
  - Shows error state appropriately
- Integration: Verify polling interval respects settings

## Out of Scope
- Click-to-expand details (future enhancement)
- Historical usage charts
- Per-project usage breakdown
