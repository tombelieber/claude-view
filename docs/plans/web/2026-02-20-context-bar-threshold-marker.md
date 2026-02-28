# Context Bar Compaction Threshold Marker

> **Date:** 2026-02-20
> **Status:** Design approved, pending implementation
> **Scope:** ContextGauge.tsx (primary), ContextBar.tsx + MonitorPane.tsx (threshold unification)

## Problem

Users monitoring live sessions in Mission Control have no visual indicator of *where* auto-compaction will trigger. The context bar fills from 0-100% but there's no reference point. Users can't intuitively tell "am I about to get compacted?" until the bar turns amber at 75% — and even then, they don't know how much room is left.

## Solution

Add a **thin vertical marker line** at ~80% on the context bar — a bullet-chart-style target marker. Users intuitively understand "don't cross this line" (fuel gauge mental model).

## Threshold Value

Claude Code's compaction threshold is **not a fixed public number**. It varies by version:

| Source | Effective Threshold |
|--------|-------------------|
| Claude API compaction default | 75% (150K trigger on 200K window) |
| Claude Code CLI buffer (33K) | ~83.5% (200K - 33K = 167K) |
| Existing codebase (`contextLimit * 0.165`) | 83.5% |

**Design choice:** Use `80%` as the marker position. It's a round number that sits between known data points, and the `~` prefix communicates approximation to users. Extracted as `AUTOCOMPACT_THRESHOLD_PCT = 80` for easy adjustment.

## Visual Specification

### Compact Mode (Session Cards)

```
Before threshold:
  ┌──────────────────────────────|──┐
  │████████████████░░░░░░░░░░░░░│░░│  72%
  └──────────────────────────────|──┘
                                 ↑
                            ~80% marker (subtle gray tick)

Past threshold:
  ┌──────────────────────────────|──┐
  │██████████████████████████████│██│  88% ⚠
  └──────────────────────────────|──┘
  Bar is red, marker becomes white for contrast
```

**Marker element:**
- `position: absolute`, `left: 80%`
- Width: `1.5px` (sharp on retina)
- Height: bar height + 2px overshoot top/bottom
- Border-radius: `rounded-full` (soft cap)
- Color transitions:
  - Before fill reaches marker: `gray-400 dark:gray-600` at `opacity-40`
  - Fill has passed marker: `white` at `opacity-90` (contrast against colored bar)
- `transition-opacity duration-300`
- Hidden when: inactive state (`needs_you` group) or compacting animation

### Expanded Mode (Side Panel)

Same marker on the stacked segmented bar, plus in the legend area:
- Small `|` glyph with label "~auto-compact" aligned to the marker position
- Consistent with existing legend items (system, conversation, buffer)

### Threshold Color Unification

Standardize all three context display components to the same thresholds:

| Zone | Range | Color | Label Visibility |
|------|-------|-------|-----------------|
| Safe | 0–75% | `sky-500` | No percentage shown |
| Caution | 75–80% | `amber-500` | Shows `XX% used` in amber |
| Danger | 80–90% | `amber-500` → `red-500` | Shows `XX% used` |
| Critical | 90–100% | `red-500` | Shows `XX% used` in red |

**Components affected:**
- `ContextGauge.tsx`: Already uses 75/90 — add 80% as marker boundary
- `ContextBar.tsx`: Change from 60/85 to 75/90 (2-line fix)
- `MonitorPane.tsx`: Change from 50/80 to 75/90 (2-line fix)

## Implementation

### Files Changed

1. **`src/components/live/ContextGauge.tsx`** — Add marker div, extract threshold constant
2. **`src/components/live/ContextBar.tsx`** — Update threshold constants (2 lines)
3. **`src/components/live/MonitorPane.tsx`** — Update threshold constants (2 lines)

### Constants

```typescript
/** Approximate context percentage at which Claude Code triggers auto-compaction. */
const AUTOCOMPACT_THRESHOLD_PCT = 80
```

### Marker Implementation (Compact Mode)

```tsx
{/* Threshold marker */}
{!isInactive && !isCompacting && (
  <div
    className={`absolute top-[-1px] bottom-[-1px] w-[1.5px] rounded-full transition-opacity duration-300 ${
      usedPct >= AUTOCOMPACT_THRESHOLD_PCT
        ? 'bg-white opacity-90'
        : 'bg-gray-400 dark:bg-gray-600 opacity-40'
    }`}
    style={{ left: `${AUTOCOMPACT_THRESHOLD_PCT}%` }}
    title="~auto-compact threshold"
  />
)}
```

The bar container needs `relative` positioning (already has it via `className="relative"`).

## What This Does NOT Do

- Does not change the tooltip content (already shows "Autocompact buffer" breakdown)
- Does not add new backend data (uses existing `contextWindowTokens` + model limits)
- Does not change the compaction behavior itself
- Does not add user-configurable threshold settings (YAGNI — put it in settings if 1% care)

## Accessibility

- Marker has `title` attribute for screen readers
- Color is not the only indicator (position on bar + percentage text)
- `prefers-reduced-motion` respected (existing `motion-safe:` prefixes maintained)
- Marker contrast: white-on-color when bar passes it (high contrast)
